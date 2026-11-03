# Reglas para agentes

- Usa Conventional Commits con descripciones en español, imperativo y minúscula.
- Haz commits atómicos; cada PR admite hasta 10 archivos y 400 líneas sin contar `Cargo.lock`.
- Ejecuta `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` y `node --test` antes de cada commit que aplique.
- Reserva stdout para eventos JSONL y escribe diagnósticos en stderr.
- Conserva los cuatro tipos de evento, `schema = 1` y el nombre libre dentro de `interaction_emit.data.name`.
- Mantén el motor sincrónico y los relojes inyectables; las pruebas no usan sleeps.
- Inserta datos del chat sólo con `textContent`; conserva BEM, variables CSS públicas y `custom.css` al final.
- Los contadores viven en `data_dir`; los assets configurados quedan bajo `overlay/assets`.
