# Criando um Servidor MCP em Rust

O **Model Context Protocol (MCP)** é um protocolo aberto desenvolvido pela Anthropic que padroniza a forma como aplicações (como LLMs) fornecem contexto e ferramentas para assistentes de IA. Neste guia, você vai aprender a **criar um servidor MCP em Rust** e **consumir** os recursos/tools expostos por ele.

---

## Índice

1. [O que é MCP?](#o-que-é-mcp)
2. [Arquitetura: Transporte JSON-RPC sobre STDIO](#arquitetura-transporte-json-rpc-sobre-stdio)
3. [Criando o Servidor MCP](#criando-o-servidor-mcp)
4. [Criando um Cliente Consumidor](#criando-um-cliente-consumidor)
5. [Exemplo Completo (Servidor + Cliente)](#exemplo-completo-servidor--cliente)

---

## O que é MCP?

MCP define três primitivas principais:

| Primitiva | Descrição |
|-----------|-----------|
| **Resources** | Dados que podem ser lidos pelo LLM (arquivos, DB queries, APIs) |
| **Tools** | Funções executáveis que o LLM pode chamar (side effects) |
| **Prompts** | Templates de mensagens pré-definidos |

A comunicação é feita via **JSON-RPC 2.0**, tradicionalmente sobre **STDIO** (pipes) ou **SSE** (Server-Sent Events).

---

## Arquitetura: Transporte JSON-RPC sobre STDIO

```
┌─────────────┐     STDIO (stdin/stdout)     ┌─────────────┐
│   Cliente   │ ◄──────────────────────────► │   Servidor  │
│  (LLM Host) │    JSON-RPC 2.0 messages     │  MCP em Rust│
└─────────────┘                               └─────────────┘
```

Cada mensagem é uma linha JSON seguida de `\n` (newline-delimited JSON).

---

## Criando o Servidor MCP

Vamos construir um servidor MCP do zero, SEM dependências externas (usando apenas `serde_json`). O servidor expõe:

- Um **resource**: `weather://cidade` – retorna clima simulado
- Uma **tool**: `get_forecast` – calcula previsão para N dias
- Um **prompt**: `weather_report` – template para gerar relatório

### 1. Setup do Projeto

```bash
cargo init mcp-weather-server
cd mcp-weather-server
cargo add serde --features derive
cargo add serde_json
```

### 2. Código do Servidor

Crie `src/main.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::collections::HashMap;

// ─── Protocol Types (JSON-RPC 2.0 + MCP) ─────────────────────────

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    result: serde_json::Value,
}

#[derive(Serialize)]
struct JsonRpcError {
    jsonrpc: String,
    id: u64,
    error: JsonRpcErrorBody,
}

#[derive(Serialize)]
struct JsonRpcErrorBody {
    code: i32,
    message: String,
}

// ─── MCP Protocol Methods ────────────────────────────────────────

fn handle_list_resources() -> serde_json::Value {
    serde_json::json!({
        "resources": [
            {
                "uri": "weather://sao-paulo",
                "name": "Clima em São Paulo",
                "description": "Temperatura e condição atual em SP",
                "mimeType": "application/json",
            },
            {
                "uri": "weather://rio-de-janeiro",
                "name": "Clima no Rio de Janeiro",
                "description": "Temperatura e condição atual no RJ",
                "mimeType": "application/json",
            },
        ]
    })
}

fn handle_read_resource(uri: &str) -> Result<serde_json::Value, JsonRpcErrorBody> {
    match uri {
        "weather://sao-paulo" => Ok(serde_json::json!({
            "temperature": 28,
            "condition": "parcialmente nublado",
            "humidity": 65,
            "city": "São Paulo",
        })),
        "weather://rio-de-janeiro" => Ok(serde_json::json!({
            "temperature": 32,
            "condition": "ensolarado",
            "humidity": 70,
            "city": "Rio de Janeiro",
        })),
        _ => Err(JsonRpcErrorBody {
            code: -32602,
            message: format!("Resource not found: {uri}"),
        }),
    }
}

fn handle_list_tools() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "get_forecast",
                "description": "Calcula previsão do tempo para os próximos N dias",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "Nome da cidade",
                        },
                        "days": {
                            "type": "integer",
                            "description": "Número de dias (1-7)",
                        },
                    },
                    "required": ["city", "days"],
                },
            }
        ]
    })
}

fn handle_call_tool(name: &str, args: &serde_json::Value) -> Result<serde_json::Value, JsonRpcErrorBody> {
    match name {
        "get_forecast" => {
            let city = args["city"].as_str().unwrap_or("desconhecida");
            let days = args["days"].as_u64().unwrap_or(1).min(7);

            let mut forecasts = Vec::new();
            let base_temp = if city.contains("Rio") { 32 } else { 28 };

            for d in 0..days {
                forecasts.push(serde_json::json!({
                    "day": format!("Day {}", d + 1),
                    "temperature": base_temp + (d as i64) - 3,
                    "condition": if d % 2 == 0 { "ensolarado" } else { "nublado" },
                }));
            }

            Ok(serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&forecasts).unwrap(),
                    }
                ],
                "isError": false,
            }))
        }
        _ => Err(JsonRpcErrorBody {
            code: -32601,
            message: format!("Tool not found: {name}"),
        }),
    }
}

fn handle_list_prompts() -> serde_json::Value {
    serde_json::json!({
        "prompts": [
            {
                "name": "weather_report",
                "description": "Gera um relatório climático",
                "arguments": [
                    {
                        "name": "city",
                        "description": "Nome da cidade",
                        "required": true,
                    }
                ],
            }
        ]
    })
}

fn handle_get_prompt(name: &str, args: &serde_json::Value) -> Result<serde_json::Value, JsonRpcErrorBody> {
    match name {
        "weather_report" => {
            let city = args["city"].as_str().unwrap_or("desconhecida");
            Ok(serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": format!(
                                "Gere um relatório climático detalhado para {city}. \
                                 Inclua temperatura, umidade, condições do vento e \
                                 recomendações para moradores."
                            ),
                        },
                    }
                ],
                "description": format!("Relatório climático para {city}"),
            }))
        }
        _ => Err(JsonRpcErrorBody {
            code: -32601,
            message: format!("Prompt not found: {name}"),
        }),
    }
}

// ─── Main Loop (JSON-RPC on STDIO) ─────────────────────────────

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(_) => break,
        };

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = JsonRpcError {
                    jsonrpc: "2.0".into(),
                    id: 0,
                    error: JsonRpcErrorBody {
                        code: -32700,
                        message: format!("Parse error: {e}"),
                    },
                };
                let _ = writeln!(stdout_lock, "{}", serde_json::to_string(&err).unwrap());
                let _ = stdout_lock.flush();
                continue;
            }
        };

        let result = match request.method.as_str() {
            "initialize" => Ok(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "resources": {},
                    "tools": {},
                    "prompts": {},
                },
                "serverInfo": {
                    "name": "mcp-weather-rust",
                    "version": "0.1.0",
                },
            })),

            "resources/list" => Ok(handle_list_resources()),
            "resources/read" => {
                let uri = request.params["uri"].as_str().unwrap_or("");
                handle_read_resource(uri)
            }

            "tools/list" => Ok(handle_list_tools()),
            "tools/call" => {
                let name = request.params["name"].as_str().unwrap_or("");
                let args = &request.params["arguments"];
                handle_call_tool(name, args)
            }

            "prompts/list" => Ok(handle_list_prompts()),
            "prompts/get" => {
                let name = request.params["name"].as_str().unwrap_or("");
                let args = &request.params["arguments"];
                handle_get_prompt(name, args)
            }

            // Ping / healthcheck
            "ping" => Ok(serde_json::json!({})),

            other => Err(JsonRpcErrorBody {
                code: -32601,
                message: format!("Method not found: {other}"),
            }),
        };

        let response = match result {
            Ok(val) => serde_json::to_string(&JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: val,
            }),
            Err(err) => serde_json::to_string(&JsonRpcError {
                jsonrpc: "2.0".into(),
                id: request.id,
                error: err,
            }),
        };

        if let Ok(json) = response {
            let _ = writeln!(stdout_lock, "{json}");
            let _ = stdout_lock.flush();
        }
    }
}
```

### 3. Testando o Servidor

```bash
cargo build --release

# Teste manual via echo:
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  | ./target/release/mcp-weather-server

echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | ./target/release/mcp-weather-server

echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_forecast","arguments":{"city":"São Paulo","days":3}}}' \
  | ./target/release/mcp-weather-server
```

Saída esperada (último comando):

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"[\n  {\n    \"day\": \"Day 1\",\n    \"temperature\": 25,\n    \"condition\": \"ensolarado\"\n  },\n  {\n    \"day\": \"Day 2\",\n    \"temperature\": 27,\n    \"condition\": \"nublado\"\n  },\n  {\n    \"day\": \"Day 3\",\n    \"temperature\": 28,\n    \"condition\": \"ensolarado\"\n  }\n]"}],"isError":false}}
```

---

## Criando um Cliente Consumidor

Agora que temos o servidor, vamos criar um **cliente MCP em Rust** que se comunica com ele via STDIO.

### 1. Setup

```bash
cargo init mcp-client
cd mcp-client
cargo add serde --features derive
cargo add serde_json
```

### 2. Código do Cliente

`src/main.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcErrorBody>,
}

#[derive(Deserialize)]
struct JsonRpcErrorBody {
    code: i32,
    message: String,
}

/// Wrapper para gerenciar um child process MCP
struct McpClient {
    process: std::process::Child,
    writer: std::io::BufWriter<std::process::ChildStdin>,
    reader: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn spawn(server_path: &str) -> Self {
        let mut process = Command::new(server_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // logs vão pra stderr do host
            .spawn()
            .expect("falhou ao spawnar servidor MCP");

        let writer = std::io::BufWriter::new(process.stdin.take().unwrap());
        let reader = BufReader::new(process.stdout.take().unwrap());

        McpClient {
            process,
            writer,
            reader,
            next_id: 1,
        }
    }

    fn send_request(&mut self, method: &str, params: Option<serde_json::Value>) -> JsonRpcResponse {
        let id = self.next_id;
        self.next_id += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        };

        let json = serde_json::to_string(&request).unwrap();
        writeln!(self.writer, "{json}").expect("falha ao escrever no stdin do servidor");
        self.writer.flush().expect("falha ao dar flush");

        let mut line = String::new();
        self.reader.read_line(&mut line).expect("falha ao ler stdout do servidor");

        serde_json::from_str(&line).expect("falha ao parsear resposta JSON-RPC")
    }

    fn initialize(&mut self) -> serde_json::Value {
        let resp = self.send_request("initialize", None);
        resp.result.expect("initialize falhou")
    }

    fn list_tools(&mut self) -> serde_json::Value {
        let resp = self.send_request("tools/list", None);
        resp.result.expect("list_tools falhou")
    }

    fn call_tool(&mut self, name: &str, args: serde_json::Value) -> serde_json::Value {
        let params = serde_json::json!({
            "name": name,
            "arguments": args,
        });
        let resp = self.send_request("tools/call", Some(params));
        resp.result.expect("call_tool falhou")
    }

    fn read_resource(&mut self, uri: &str) -> serde_json::Value {
        let params = serde_json::json!({ "uri": uri });
        let resp = self.send_request("resources/read", Some(params));
        resp.result.expect("read_resource falhou")
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

// ─── Exemplo de uso ──────────────────────────────────────────────

fn main() {
    let server_path = "./target/release/mcp-weather-server";

    println!("🚀 Spawnando servidor MCP: {server_path}");
    let mut client = McpClient::spawn(server_path);

    // 1. Inicializar
    let info = client.initialize();
    println!("✅ Servidor conectado: {}", info["serverInfo"]["name"]);

    // 2. Listar ferramentas
    let tools = client.list_tools();
    println!("🔧 Ferramentas disponíveis: {}", tools["tools"]);

    // 3. Chamar uma tool
    let forecast = client.call_tool("get_forecast", serde_json::json!({
        "city": "Rio de Janeiro",
        "days": 5,
    }));
    println!("🌤️  Previsão: {}", forecast["content"][0]["text"]);

    // 4. Ler um resource
    let clima_sp = client.read_resource("weather://sao-paulo");
    println!("📍 Clima SP: {}°C - {}",
        clima_sp["temperature"],
        clima_sp["condition"],
    );

    println!("✅ Cliente finalizado com sucesso!");
}
```

### 3. Executando

```bash
# Terminal 1: tenha o servidor compilado
cd mcp-weather-server && cargo build --release

# Terminal 2: execute o cliente
cd mcp-client && cargo run
```

---

## Usando com LLMs (ex: Claude Desktop)

Para usar seu servidor MCP com **Claude Desktop** (ou qualquer host MCP), crie ou edite o arquivo de configuração:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "weather-rust": {
      "command": "/caminho/absoluto/para/mcp-weather-server/target/release/mcp-weather-server",
      "args": []
    }
  }
}
```

Após reiniciar o Claude Desktop, o servidor aparecerá com suas tools, resources e prompts prontos para uso.

---

## Resumo

| Componente | O que fizemos |
|------------|---------------|
| **Servidor** | Implementamos JSON-RPC 2.0 sobre STDIO com suporte a resources, tools e prompts |
| **Cliente** | Wrapper Rust que spawna o servidor como child process e faz chamadas |
| **Integração** | Configuração para Claude Desktop consumir o servidor |

O MCP é um protocolo simples mas poderoso — você pode estender este exemplo para conectar bancos de dados, APIs REST, sistemas de arquivos ou qualquer ferramenta que seu LLM precise acessar.

---

## Próximos Passos

- Adicione **transport via SSE** (HTTP) para servidores remotos
- Use a crate oficial [`modelcontextprotocol`](https://crates.io/crates/modelcontextprotocol) quando sair da alpha
- Implemente **notificações** (`notifications/`) para updates em tempo real
- Adicione **logging** via stderr (o protocolo reserva stdout para JSON-RPC)
