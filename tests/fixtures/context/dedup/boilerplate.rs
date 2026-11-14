// Fixture dourado do dedup contíguo (PR38, onda TE).
//
// Imports duplicados em bloco: exatamente o lixo que o packer levava linha
// a linha para o prompt. O termo de busca do teste casa PRIMEIRO com as
// três linhas de import abaixo — é esse run contíguo que o dedup colapsa.
use crate::auth::session_token;
use crate::auth::session_token;
use crate::auth::session_token;

pub fn rotate() -> bool {
    let token = session_token();
    if token.is_empty() {
        return false;
    }
    true
}
