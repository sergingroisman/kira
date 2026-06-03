# Notas sobre Rust 🦀

## O que é Rust?

Rust é uma linguagem de programação de sistemas que prioriza **segurança**, **performance** e **concorrência**. Foi criada por Graydon Hoare na Mozilla Research e lançada publicamente em 2015. Atualmente é mantida pela Rust Foundation.

## Principais Características

### 1. Ownership (Propriedade)
O sistema de *ownership* é o diferencial mais marcante de Rust. Cada valor tem **exatamente um dono** em qualquer momento:

```rust
let s1 = String::from("hello"); // s1 é dono da String
let s2 = s1;                     // posse movida para s2
// println!("{s1}");             // erro! s1 não é mais dono
```

### 2. Borrowing (Empréstimo)
Em vez de mover, podemos *emprestar* uma referência:

```rust
fn calcular_tamanho(s: &String) -> usize { // & = referência imutável
    s.len()
} // s sai de escopo, mas não desaloca a String original
```

- `&T` — referência imutável (múltiplas permitidas)
- `&mut T` — referência mutável (apenas uma por vez)

### 3. Lifetimes (Tempos de Vida)
O *borrow checker* garante que referências nunca sejam usadas após o dado ser liberado:

```rust
fn maior<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

### 4. Pattern Matching
`match` e `if let` são onipresentes:

```rust
match valor {
    1 => println!("um"),
    2 | 3 => println!("dois ou três"),
    4..=10 => println!("entre 4 e 10"),
    _ => println!("outro"),
}
```

### 5. Sistema de Tipos Robusto
- **Tipos algébricos:** `enum` com dados embutidos (algebraic data types)
- **Option<T>** — substitui `null` de forma segura
- **Result<T, E>** — tratamento explícito de erros sem exceções
- **Traits** — equivalente a interfaces, com suporte a métodos padrão

```rust
fn dividir(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err("divisão por zero".to_string())
    } else {
        Ok(a / b)
    }
}
```

### 6. Concorrência sem Data Races
Rust garante *thread safety* em tempo de compilação via `Send` e `Sync`:

```rust
use std::sync::Mutex;
use std::thread;

let contador = std::sync::Arc::new(Mutex::new(0));
let mut handles = vec![];

for _ in 0..10 {
    let c = std::sync::Arc::clone(&contador);
    handles.push(thread::spawn(move || {
        let mut num = c.lock().unwrap();
        *num += 1;
    }));
}
```

## Ferramentas do Ecossistema

| Ferramenta   | Função                              |
|-------------|--------------------------------------|
| `rustup`    | Gerenciador de versões do Rust       |
| `cargo`     | Build system + gerenciador de pacotes|
| `rustfmt`   | Formatador de código                 |
| `clippy`    | Linter com sugestões                 |
| `rust-analyzer` | LSP para IDEs                    |

## Comandos básicos do Cargo

```bash
cargo new meu_projeto     # cria novo projeto
cargo build               # compila (debug)
cargo build --release     # compila (otimizado)
cargo run                 # executa
cargo test                # roda testes
cargo check               # verifica sem gerar binário (rápido)
cargo add serde           # adiciona dependência
```

## Por que usar Rust?

✅ **Performance comparável a C/C++** — sem garbage collector, zero-cost abstractions  
✅ **Garantias de memória em tempo de compilação** — sem segfaults, sem double-free  
✅ **Documentação e comunidade excelentes** — *The Book*, *Rust by Example*, Discord, Reddit  
✅ **Adoção crescente** — Linux kernel, Firefox, Dropbox, Cloudflare, AWS  

## Recursos para Aprender

- [The Rust Programming Language (The Book)](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/stable/rust-by-example/)
- [Rustlings](https://github.com/rust-lang/rustlings) — exercícios interativos
- [Tour of Rust](https://tourofrust.com/) — tutorial visual
- [This Week in Rust](https://this-week-in-rust.org/) — newsletter semanal
