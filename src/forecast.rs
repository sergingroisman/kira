//! Engine de forecasting DETERMINÍSTICA (sem LLM) — tendência linear com
//! banda de confiança e detecção de anomalias por z-score.
//!
//! O Timestral (mistralai/Timestral-209M) EXISTE e é forte (SOTA zero-shot em
//! GIFT-Eval/TIME/fev-bench), mas é PyTorch/safetensors com inferência em
//! Python — NÃO é um GGUF do Ollama, e nenhum endpoint do Ollama produz
//! quantis de série temporal. Prometer "forecasting via Ollama" seria mentira
//! acionável; por isso a engine da casa é estatística pura (cálculo determinístico,
//! zero custo, zero rede — a camada 3 do ADR-0016) e o Timestral fica
//! documentado como sidecar Python futuro (ADR-0042).
//!
//! Usecases no KIRA:
//! - Velocity forecasting por sprint
//! - Recorrência de bugs (sazonalidade grossa via z-score)
//! - Planejamento de capacidade

use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Ponto de dados da série temporal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePoint {
    /// Timestamp (ISO 8601; a data `YYYY-MM-DD` basta para o engine).
    pub timestamp: String,
    /// Valor numérico.
    pub value: f64,
    /// Metadados opcionais (ex: sprint_id, assignee).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Série temporal completa. O engine NÃO reordena: os pontos entram na ordem
/// que o caller passar (ordenar por timestamp é responsabilidade de quem
/// extrai — a ordem define o índice da regressão).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    /// Pontos de dados NA ORDEM DO TEMPO.
    pub points: Vec<TimePoint>,
    /// Frequência dos dados, em dias entre pontos consecutivos.
    pub frequency_days: u32,
    /// Metadados da série (ex: projeto, sprint, tipo).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Previsão com banda de quantis (p10/p50/p90).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forecast {
    /// Valores previstos para cada passo do horizonte (mediana/p50).
    pub predictions: Vec<f64>,
    /// Quantil p10 ( banda inferior).
    pub lower: Vec<f64>,
    /// Quantil p90 (banda superior).
    pub upper: Vec<f64>,
    /// Timestamps previstos (ISO 8601, `...T00:00:00Z`), quando o último
    /// timestamp da série é uma data legível — senão vazio (nunca inventado).
    pub timestamps: Vec<String>,
    /// Quantis calculados.
    pub quantiles: Vec<f32>,
    /// Métricas de qualidade/auditabilidade da regressão.
    #[serde(default)]
    pub metrics: HashMap<String, f64>,
}

/// Anomalia detectada na série.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Timestamp do ponto anômalo (o do PRÓPRIO ponto, nunca "point_N").
    pub timestamp: String,
    /// Valor observado.
    pub observed: f64,
    /// Valor esperado (média da série).
    pub expected: f64,
    /// Desvio em desvios-padrão (z-score absoluto).
    pub deviation: f64,
    /// Severidade (0.0 a 1.0): z/5 saturado — z ≥ 5 é severidade máxima.
    pub severity: f32,
    /// Tipo de anomalia.
    pub kind: AnomalyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyKind {
    /// Pico inesperado (spike).
    Spike,
    /// Queda brusca (drop).
    Drop,
}

/// Horizonte máximo: previsão de mais de um ano adiantado é ruído, não decisão.
const MAX_HORIZON: usize = 365;

/// z(0.10) ≈ z(0.90) da normal — banda bilateral aproximada da engine.
const Z_BAND: f64 = 1.2816;

/// Previsão por regressão linear + banda de residuais.
///
/// - p50 = linha da regressão.
/// - p10/p90 = linha ∓ z(0.10)·σ(residuais) — banda CONSTANTE, aproximação
///   honesta documentada (não é modelo probabilístico completo; para isso vem
///   o sidecar Timestral).
/// - A média móvel e a tendência pura ficam expostas como helpers
///   (`forecast_moving_average`/`forecast_linear_trend`).
///
/// Erro acionável com < 2 pontos ou > MAX_HORIZON: a engine nunca devolve
/// forecast fingido.
pub fn forecast(series: &TimeSeries, horizon: usize) -> Result<Forecast, Error> {
    if horizon == 0 || horizon > MAX_HORIZON {
        return Err(Error::Forecast(format!(
            "horizonte {horizon} inválido — use 1..={MAX_HORIZON} passos"
        )));
    }
    let values: Vec<f64> = series.points.iter().map(|p| p.value).collect();
    if values.len() < 2 {
        return Err(Error::Forecast(format!(
            "a série tem {} ponto(s) — o forecast precisa de pelo menos 2 (extraia mais histórico antes)",
            values.len()
        )));
    }
    let predictions = forecast_linear_trend(&values, horizon);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    // Residuais contra a LINHA da regressão (não contra a média): a banda tem
    // que medir o que a tendência não explica, senão séries lineares ganham
    // banda falsa.
    let slope = regression_slope(&values).unwrap_or(0.0);
    let n = values.len() as f64;
    let intercept = mean - slope * ((n - 1.0) / 2.0);
    let residual_var = values
        .iter()
        .enumerate()
        .map(|(i, &v)| (v - (intercept + slope * i as f64)).powi(2))
        .sum::<f64>()
        / n;
    let residual_std = residual_var.sqrt();
    let band = Z_BAND * residual_std;
    let timestamps = future_timestamps(series, horizon, series.frequency_days);
    let mut metrics = HashMap::new();
    metrics.insert("residual_std".to_string(), residual_std);
    metrics.insert("slope".to_string(), slope);
    Ok(Forecast {
        lower: predictions.iter().map(|p| p - band).collect(),
        upper: predictions.iter().map(|p| p + band).collect(),
        predictions,
        timestamps,
        quantiles: vec![0.1, 0.5, 0.9],
        metrics,
    })
}

/// Slope da regressão sobre os índices 0..n — a inclinação que o p50 segue.
fn regression_slope(values: &[f64]) -> Option<f64> {
    let n = values.len() as f64;
    if n < 2.0 {
        return None;
    }
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = values.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &y) in values.iter().enumerate() {
        let x = i as f64;
        num += (x - x_mean) * (y - y_mean);
        den += (x - x_mean).powi(2);
    }
    (den != 0.0).then(|| num / den)
}

/// Detecção de anomalias por z-score sobre a série INTEIRA (regra
/// estatística, sem LLM). Menos de 10 pontos não tem estatística suficiente
/// para z-score honesto — devolve vazio (e não é erro: séries novas são normais).
#[must_use]
pub fn detect_anomalies(series: &TimeSeries, threshold_std: f64) -> Vec<Anomaly> {
    let values: Vec<f64> = series.points.iter().map(|p| p.value).collect();
    detect_anomalies_values(&values, threshold_std, |i| {
        series.points.get(i).map(|p| p.timestamp.clone())
    })
}

/// Núcleo puro do z-score (testável sem montar `TimeSeries`).
fn detect_anomalies_values(
    values: &[f64],
    threshold_std: f64,
    timestamp_of: impl Fn(usize) -> Option<String>,
) -> Vec<Anomaly> {
    if values.len() < 10 {
        return vec![];
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let std = variance.sqrt();
    values
        .iter()
        .enumerate()
        .filter_map(|(i, &value)| {
            let zscore = (value - mean) / std.max(1e-10);
            (zscore.abs() > threshold_std).then(|| Anomaly {
                timestamp: timestamp_of(i).unwrap_or_else(|| format!("point_{i}")),
                observed: value,
                expected: mean,
                deviation: zscore.abs(),
                severity: ((zscore.abs() / 5.0).min(1.0)) as f32,
                kind: if value > mean {
                    AnomalyKind::Spike
                } else {
                    AnomalyKind::Drop
                },
            })
        })
        .collect()
}

/// Previsão por média móvel (última janela constante).
#[must_use]
pub fn forecast_moving_average(series: &[f64], horizon: usize, window: usize) -> Vec<f64> {
    if series.is_empty() || window == 0 {
        return vec![0.0; horizon];
    }
    let w = window.min(series.len());
    let ma: f64 = series[series.len() - w..].iter().sum::<f64>() / w as f64;
    vec![ma; horizon]
}

/// Previsão por tendência linear — RESPEITA o `horizon` (o parâmetro é o
/// contrato; devolver tamanho fixo era bug da versão anterior).
#[must_use]
pub fn forecast_linear_trend(series: &[f64], horizon: usize) -> Vec<f64> {
    if series.len() < 2 {
        return vec![0.0; horizon];
    }
    let n = series.len() as f64;
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = series.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &y) in series.iter().enumerate() {
        let x = i as f64;
        num += (x - x_mean) * (y - y_mean);
        den += (x - x_mean).powi(2);
    }
    let slope = if den != 0.0 { num / den } else { 0.0 };
    let intercept = y_mean - slope * x_mean;
    (1..=horizon)
        .map(|i| intercept + slope * (n + i as f64 - 1.0))
        .collect()
}

/// Timestamps futuros: último ponto + i·frequência, em ISO 8601 UTC. A data
/// nasce de aritmética civil própria (dias desde a era → ano/mês/dia), SEM
/// dependência externa — o repo evita chrono por política (o store já faz o
/// mesmo em `now_rfc3339_utc`).
fn future_timestamps(series: &TimeSeries, horizon: usize, frequency_days: u32) -> Vec<String> {
    let Some(last) = series.points.last() else {
        return vec![];
    };
    let Some(last_days) = civil_days_of(&last.timestamp) else {
        // Timestamp ilegível: vazio e VISÍVEL (o caller vê timestamps vazio
        // em vez de receber datas inventadas).
        return vec![];
    };
    let freq = frequency_days.max(1) as i64;
    (1..=horizon)
        .map(|i| {
            let days = last_days + freq * i as i64;
            let (y, m, d) = civil_from_days(days);
            format!("{y:04}-{m:02}-{d:02}T00:00:00Z")
        })
        .collect()
}

/// Extrai os primeiros `YYYY-MM-DD` de um timestamp e devolve dias desde
/// 1970-01-01 (algoritmo days_from_civil — Howard Hinnant, domínio público).
fn civil_days_of(timestamp: &str) -> Option<i64> {
    let bytes = timestamp.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    let year: i64 = timestamp.get(0..4)?.parse().ok()?;
    let month: i64 = timestamp.get(5..7)?.parse().ok()?;
    let day: i64 = timestamp.get(8..10)?.parse().ok()?;
    if !(bytes[4] == b'-'
        && bytes[7] == b'-'
        && (1..=12).contains(&month)
        && (1..=31).contains(&day))
    {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// Dias desde 1970-01-01 (Howard Hinnant, `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + (if m > 2 { -3 } else { 9 })) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Ano/mês/dia desde dias da era (Howard Hinnant, `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + (if mp < 10 { 3 } else { -9 });
    (y + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(values: &[f64], last_ts: &str, freq: u32) -> TimeSeries {
        // Timestamps dos pontos históricos: derivados para trás a partir do
        // último (só o último precisa ser legível para os futuros).
        let last_days = civil_days_of(last_ts).expect("timestamp de teste legível");
        let freq_i = freq.max(1) as i64;
        let points = values
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let days = last_days - freq_i * (values.len() - 1 - i) as i64;
                let (y, m, d) = civil_from_days(days);
                TimePoint {
                    timestamp: format!("{y:04}-{m:02}-{d:02}T00:00:00Z"),
                    value,
                    metadata: HashMap::new(),
                }
            })
            .collect();
        TimeSeries {
            points,
            frequency_days: freq,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn forecast_respects_horizon_and_matches_trend() {
        // Série perfeitamente linear 1..5 ⇒ p50 continua 6,7,8...
        let s = series(&[1.0, 2.0, 3.0, 4.0, 5.0], "2026-11-16T00:00:00Z", 1);
        let forecast = forecast(&s, 3).expect("forecast de série válida");
        assert_eq!(forecast.predictions.len(), 3, "horizonte é o contrato");
        assert!((forecast.predictions[0] - 6.0).abs() < 0.1, "{forecast:?}");
        assert!((forecast.predictions[2] - 8.0).abs() < 0.1);
        assert_eq!(forecast.quantiles, vec![0.1, 0.5, 0.9]);
        assert_eq!(forecast.timestamps.len(), 3);
        assert_eq!(
            forecast.timestamps[0], "2026-11-17T00:00:00Z",
            "{:?}",
            forecast.timestamps
        );
        assert_eq!(forecast.timestamps[2], "2026-11-19T00:00:00Z");
        // Série linear ⇒ residual_std = 0 ⇒ banda colapsa no p50 (honesto: a
        // tendência explica tudo; nenhuma incerteza inventada).
        for i in 0..3 {
            assert!((forecast.lower[i] - forecast.predictions[i]).abs() < 1e-9);
            assert!((forecast.upper[i] - forecast.predictions[i]).abs() < 1e-9);
        }
        assert!(
            (forecast.metrics["residual_std"]).abs() < 1e-9,
            "série linear tem residual 0"
        );
    }

    #[test]
    fn forecast_band_opens_with_residual_noise() {
        // Série com ruído real: 1,2,3,5,5 — a tendência não explica tudo e a
        // banda p10/p90 tem que abrir de verdade.
        let s = series(&[1.0, 2.0, 3.0, 5.0, 5.0], "2026-11-16T00:00:00Z", 1);
        let forecast = forecast(&s, 2).expect("forecast de série válida");
        assert!(forecast.metrics["residual_std"] > 0.0);
        assert!(forecast.lower[0] < forecast.predictions[0]);
        assert!(forecast.upper[0] > forecast.predictions[0]);
        // Banda simétrica: upper - p50 == p50 - lower (z único, banda fixa).
        assert!(
            ((forecast.upper[0] - forecast.predictions[0])
                - (forecast.predictions[0] - forecast.lower[0]))
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn forecast_rejects_short_series_and_bad_horizon() {
        let one = series(&[5.0], "2026-11-16T00:00:00Z", 1);
        let Err(err) = forecast(&one, 3) else {
            panic!("1 ponto deve virar erro acionável");
        };
        assert!(err.to_string().contains("pelo menos 2"), "{err}");
        let two = series(&[1.0, 2.0], "2026-11-16T00:00:00Z", 1);
        let Err(err) = forecast(&two, 0) else {
            panic!("horizonte 0 deve virar erro");
        };
        assert!(err.to_string().contains("horizonte"), "{err}");
        let Err(_) = forecast(&two, MAX_HORIZON + 1) else {
            panic!("horizonte acima do teto deve virar erro");
        };
    }

    #[test]
    fn forecast_with_unreadable_timestamps_leaves_timestamps_empty() {
        let points = vec![
            TimePoint {
                timestamp: "sem-data".into(),
                value: 1.0,
                metadata: HashMap::new(),
            },
            TimePoint {
                timestamp: "sem-data".into(),
                value: 2.0,
                metadata: HashMap::new(),
            },
        ];
        let s = TimeSeries {
            points,
            frequency_days: 1,
            metadata: HashMap::new(),
        };
        let forecast = forecast(&s, 2).expect("valores são suficientes para o forecast");
        assert!(
            forecast.timestamps.is_empty(),
            "não inventa data: {forecast:?}"
        );
        assert_eq!(forecast.predictions.len(), 2);
    }

    #[test]
    fn anomalies_carry_the_real_timestamp_and_kind() {
        // Fixture com folga REAL dos dois lados: média 125, std ≈ 264 —
        // z(spike) ≈ z(drop) ≈ 2,45 > 2 (um spike enorme sozinho infla o std
        // e mata o drop; os dois desvios têm que coexistir acima do limiar).
        let mut values = vec![100.0; 12];
        values[5] = 1000.0; // spike
        values[9] = -800.0; // drop
        let s = series(&values, "2026-11-16T00:00:00Z", 1);
        let anomalies = detect_anomalies(&s, 2.0);
        assert_eq!(anomalies.len(), 2, "{anomalies:?}");
        assert_eq!(anomalies[0].kind, AnomalyKind::Spike);
        assert!(
            !anomalies[0].timestamp.starts_with("point_"),
            "timestamp real: {}",
            anomalies[0].timestamp
        );
        assert_eq!(anomalies[1].kind, AnomalyKind::Drop);
    }

    #[test]
    fn anomalies_needs_enough_points() {
        let short = vec![10.0, 10.0, 100.0];
        assert!(detect_anomalies(&series(&short, "2026-11-16", 1), 2.0).is_empty());
    }

    #[test]
    fn moving_average_and_trend_helpers() {
        let values = vec![10.0, 10.0, 10.0, 10.0];
        let ma = forecast_moving_average(&values, 5, 3);
        assert_eq!(ma.len(), 5);
        assert!((ma[0] - 10.0).abs() < 1e-6);
        let trend = forecast_linear_trend(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        assert_eq!(trend.len(), 3, "o horizon é respeitado");
        assert!((trend[0] - 6.0).abs() < 0.1);
    }

    #[test]
    fn civil_date_math_round_trips_known_dates() {
        // Âncoras conhecidas: 1970-01-01 = dia 0; 2024 é bissexto.
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(
            days_from_civil(2024, 2, 29),
            19_782,
            "29/fev/2024 (âncora: 19782)"
        );
        let (y, m, d) = civil_from_days(19_782);
        assert_eq!((y, m, d), (2024, 2, 29));
        // 2026-11-16 — a data "hoje" do projeto como âncora da conversa.
        let days = days_from_civil(2026, 11, 16);
        assert_eq!(civil_from_days(days), (2026, 11, 16));
        assert_eq!(civil_days_of("2026-11-16T09:00:00Z"), Some(days));
        assert_eq!(civil_days_of("2026-11-16"), Some(days));
        assert_eq!(civil_days_of("16/11/2026"), None);
        assert_eq!(civil_days_of("2026-13-01"), None);
        assert_eq!(civil_days_of("curto"), None);
    }
}
