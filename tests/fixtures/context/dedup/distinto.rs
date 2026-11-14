// Fixture do candidato RECUSADO (PR38, onda TE): linhas que casam com o
// mesmo termo de busca, mas sem repetição nenhuma — nem dentro do arquivo,
// nem entre arquivos (o import usa alias de propósito: o texto NÃO é igual
// ao do `boilerplate.rs`). É sobre a lista combinada que o teste mede o
// dedup ENTRE ARQUIVOS e prova que ele fica fora por não bater a barra.
use crate::auth::session_token as token;

pub fn audit() -> usize {
    let session_token = token();
    session_token.len()
}

pub fn refresh() -> bool {
    token().rotate()
}
