# Interactions

Plugin oficial de `streams-tools` para interacciones del chat: comandos con texto, imagen y sonido, secuencias de moderadores, combos de moderadores, combos del chat (palabras o emotes repetidos), contadores persistentes y reacciones a canjes y donaciones.

El plugin **sólo lee** eventos. No escribe en el chat, no responde comandos con mensajes y no declara permisos de escritura. Todo lo visible ocurre en el overlay.

## Compatibilidad

| Plataforma | Chat, comandos y combos | Canjes (`redemption`) | Donaciones (`donation`) |
|---|---|---|---|
| Twitch | Sí | Sí | Sí (bits) |
| Kick | Sí | No (el adaptador no los emite) | No (el adaptador no los emite) |
| YouTube | Sí | No | Sí (Super Chat) |

Los roles que entregan los tres adaptadores son `broadcaster`, `moderator`, `vip` y `subscriber`.

## Instalación

Añade el repositorio a `config.toml` de streams-tools para instalar el plugin cuando falte:

```toml
[plugins]
install = ["https://github.com/belcaik/streams-plugin-interactions"]
```

También puedes instalarlo desde la línea de comandos:

```sh
streams-tools plugin add https://github.com/belcaik/streams-plugin-interactions
```

Ambos métodos descargan el ZIP apropiado para la plataforma y verifican su SHA-256. Para fijar la versión usa `https://github.com/belcaik/streams-plugin-interactions@v0.1.0`.

### OBS

Añade un Browser Source con URL `http://127.0.0.1:7777/p/interactions/` y tamaño 1920×1080. El fondo es transparente; los efectos se centran y los contadores van a la esquina inferior derecha.

## Estructura

```text
interactions/
├── plugin.toml               # permisos, worker y punteros de configuración
├── config/interactions.toml  # reglas: comandos, combos, contadores, triggers
├── bin/interactions          # ejecutable compilado (no versionado)
├── src/                      # worker Rust
├── tests/                    # pruebas Rust (caja negra) y Node
└── overlay/
    ├── index.html, app.js, interactions-core.js, style.css
    ├── test.html, test.css   # página de prueba
    └── assets/hype.svg       # assets referenciables desde las reglas
```

## Configuración y estado

- `plugin.toml`: contiene los defaults de fábrica y se reemplaza al actualizar; no lo edites.
- `config/interactions.toml`: contiene las reglas incluidas en el paquete. No es almacenamiento persistente del usuario y se reemplaza al actualizar.
- `data/interactions/counters.json`: estado mutable de los contadores, en el `data_dir` que asigna el host (`STREAMS_DATA_DIR`, por defecto `./data`). Nunca se guarda estado junto al plugin.

Configura el plugin en el `config.toml` principal. El host hace deep merge de `[plugins.config.interactions]` sobre `[config]` del manifiesto; sólo necesitas declarar los overrides:

```toml
[plugins.config.interactions]
max_queue = 50
gap_ms = 250
rules_file = "/ruta/persistente/interactions.toml"
```

Para reglas propias, copia primero el `config/interactions.toml` incluido a una ruta persistente y reemplaza `rules_file` como en el ejemplo. Así una actualización no borra tus reglas. No hay recarga en caliente: reinicia streams-tools después de cambiar la configuración o las reglas.

Defaults disponibles en `[config]` del manifiesto:

| Clave | Default | Descripción |
|---|---|---|
| `rules_file` | `"config/interactions.toml"` | Reglas, relativas a la raíz del plugin. |
| `assets_dir` | `"overlay/assets"` | Carpeta donde el worker valida assets. |
| `asset_url` | `"./assets/"` | Prefijo de URL para el overlay; debe empezar por `./assets/` y acabar en `/`. |
| `max_queue` | `30` | Efectos pendientes en el overlay (1..100). Al llenarse se descarta el nuevo. |
| `default_duration_ms` | `4000` | Duración si un efecto no la define (100..300000). |
| `gap_ms` | `150` | Pausa entre efectos (0..60000). |

## Reglas

El archivo incluido [`config/interactions.toml`](config/interactions.toml) es el ejemplo completo y la fuente de verdad del formato. Los campos desconocidos invalidan la configuración y el worker termina con un error `ruta: campo X: motivo` antes de procesar el chat.

### `[[commands]]`

| Campo | Default | Descripción |
|---|---|---|
| `name` | obligatorio | Nombre del comando sin `!`. |
| `aliases` | `[]` | Nombres alternativos. |
| `roles` | `["everyone"]` | Roles que pueden usarlo. |
| `platforms` | las tres | `twitch`, `kick`, `youtube`; una lista vacía es error. |
| `global_cooldown_ms` | `0` | Espera entre usos de cualquier usuario. |
| `user_cooldown_ms` | `0` | Espera por usuario y plataforma. |
| `[[commands.steps]]` | `[]` | Efectos visuales en orden. |
| `[[commands.emits]]` | `[]` | Eventos libres para otros plugins. |

Cada regla necesita al menos un paso o una emisión.

### `[[mod_actions]]`

Igual que un comando, pero sin `roles`: siempre exige `moderator` o `broadcaster`. Sus `steps` se encolan en el orden del TOML (`!hype`, `!shoutout usuario`, `!countdown 60`). Opcionalmente lleva un combo:

| Campo de `[mod_actions.combo]` | Rango | Descripción |
|---|---|---|
| `threshold` | 2..100 | Moderadores distintos necesarios. |
| `window_ms` | 100..300000 | Ventana de tiempo. |
| campos de efecto | | Efecto especial (ver tabla de efecto). |

### `[[chat_combos]]`

| Campo | Default | Descripción |
|---|---|---|
| `name` | obligatorio | Identificador de la regla. |
| `platforms` | las tres | Plataformas vigiladas. |
| `threshold` | obligatorio, 2..100 | Usuarios distintos para disparar. |
| `window_ms` | obligatorio, 100..300000 | Ventana de tiempo. |
| `case_sensitive` | `false` | Distinguir mayúsculas ASCII. |
| `min_token_length` | `1` (1..64) | Longitud mínima de palabra. |
| `ignore_prefixes` | `[]` | Mensajes que empiezan así no cuentan (por ejemplo `"!"`). |
| campos de efecto | | Admite además `{key}`. |

### `[[counters]]`

| Campo | Default | Descripción |
|---|---|---|
| `name` | obligatorio | Clave en `counters.json`. |
| `show_command` | obligatorio | Comando de lectura, abierto a todos. |
| `increment_command` | obligatorio | Suma 1. |
| `decrement_command` | ninguno | Resta 1. |
| `initial` | obligatorio | Valor inicial; debe estar entre `min` y `max`. |
| `min` / `max` | límites de `i64` | El valor satura en estos límites. |
| `mutation_roles` | `["moderator", "broadcaster"]` | Roles que pueden mutar. |
| campos de efecto | | Admite además `{name}` y `{value}`. |

Los comandos de contadores no tienen cooldown ni alias.

### `[[event_triggers]]`

| Campo | Default | Descripción |
|---|---|---|
| `name` | obligatorio | Identificador. |
| `enabled` | `true` | Desactiva la regla sin borrarla. |
| `event` | obligatorio | `redemption` o `donation`. |
| `platforms` | las tres | Plataformas aceptadas. |
| `reward_id`, `reward_title` | ninguno | Canjes: al menos uno; igualdad exacta. |
| `currency` | ninguno | Donaciones: moneda, sin distinguir mayúsculas ASCII. |
| `min_amount`, `max_amount` | ninguno | Donaciones: importes finitos, no negativos, `min <= max`. |
| `steps`, `emits` | `[]` | Como en comandos. |

Se ejecutan todas las reglas que coinciden, en orden. En ellas `{args}` es el `input` del canje o el `message` de la donación, y `{command}` es el `reward_title` o `donation`.

### Efecto (`steps`, `combo`, `chat_combos`, `counters`)

| Campo | Default | Descripción |
|---|---|---|
| `text` | ninguno | Texto plano con placeholders. |
| `image` | ninguno | Ruta relativa a `overlay/assets`. |
| `sound` | ninguno | Ruta relativa a `overlay/assets`. |
| `variant` | `"default"` | Clase visual: `default`, `fullscreen`, `combo`, `counter` o una propia (`^[a-z0-9][a-z0-9-]*$`). |
| `duration_ms` | `default_duration_ms` | 100..300000. |

### `[[...emits]]`

| Campo | Default | Descripción |
|---|---|---|
| `name` | obligatorio | Nombre libre (`^[a-z0-9][a-z0-9_-]*$`). |
| `payload` | `{}` | Cualquier valor TOML, copiado tal cual. |

## Placeholders

| Placeholder | Valor |
|---|---|
| `{user}` | Nombre visible del usuario. |
| `{args}` | Texto tras el comando, sin espacios en los extremos. |
| `{target}` | Primer argumento o vacío. |
| `{count}` | Ejecuciones aceptadas de la regla desde que arrancó el worker; en combos, el total actual. |
| `{platform}` | `twitch`, `kick` o `youtube`. |
| `{command}` | Comando o alias usado, en minúsculas. |
| `{key}` | Sólo `chat_combos`: palabra del combo. |
| `{name}`, `{value}` | Sólo `counters`. |

Un placeholder desconocido o llaves sin balancear invalidan la configuración. El resultado siempre es texto plano.

## Comandos, roles y cooldowns

- Un comando empieza exactamente en el primer carácter con `!`; `  !hola` no cuenta. El nombre termina en el primer espacio y se compara en minúsculas ASCII; los argumentos conservan Unicode.
- Nombres y alias usan minúsculas ASCII, dígitos, `_`, `-` y `+`. Comandos, alias, acciones mod y comandos de contadores comparten un único espacio de nombres sin duplicados.
- Roles: `broadcaster`, `moderator`, `vip`, `subscriber`, `everyone` (este último solo). Una regla coincide si incluye `everyone`, si el usuario tiene uno de sus roles o si pide `moderator` y el usuario es `broadcaster`.
- Primero se comprueban plataforma y rol, después el cooldown. El cooldown sólo se registra si la regla se ejecuta. La identidad es plataforma + id de usuario: las plataformas no comparten cooldown. Un rechazo no emite nada.

## Algoritmos de combos

**Combo de moderadores.** Por plataforma y acción, cada ejecución aceptada añade al moderador (por `user.id`) si no estaba en la ventana y descarta los que salieron de ella. Al llegar a `threshold` se emite un único `interaction_combo` y el contador se vacía. Un moderador repetido no suma. La acción normal se ejecuta igualmente en cada mensaje.

**Combo del chat.** Para cada mensaje que no empieza por un prefijo ignorado: se separa por espacios, se recorta la puntuación ASCII de los extremos, se descartan palabras cortas y repetidas dentro del mismo mensaje y se normalizan mayúsculas si `case_sensitive=false`. Por palabra se guarda la última aparición de cada usuario dentro de la ventana. Cuando un usuario nuevo deja el total `>= threshold` se emite `x5`, y después cada usuario nuevo emite el total actual (`x6`, `x7`...). Si la ventana baja del umbral la racha se apaga sin emitir nada. `key` conserva la grafía del mensaje más reciente. Las plataformas nunca se mezclan. Un comando del chat puede contar si ningún `ignore_prefixes` lo excluye.

## Contadores

La lectura es pública; las mutaciones exigen `mutation_roles`. El valor nuevo se guarda primero en disco y sólo después se emite `interaction_counter`; si la escritura falla se registra en stderr y no se anuncia. El formato es estable:

```json
{"version":1,"values":{"muertes":7}}
```

Las claves que ya no están configuradas se conservan, así que quitar y restaurar una regla mantiene su valor. Un JSON roto, una versión distinta de 1 o un valor fuera de `i64` detienen el worker con un error claro. La escritura usa `counters.json.tmp` y un renombrado atómico.

## Contrato de eventos

Todos los payloads llevan `schema: 1`; `text`, `image` y `sound` siempre aparecen (string o `null`). El host añade `id`, `platform = "plugin:interactions"` y `ts`.

```json
{"type":"interaction_show","data":{"schema":1,"source":"command","rule":"hello","step":0,"text":"Hola Ana","image":null,"sound":null,"variant":"default","duration_ms":4000},"user":{"id":"u1","name":"Ana","roles":[]}}
{"type":"interaction_combo","data":{"schema":1,"kind":"moderators","rule":"hype","key":"hype","count":3,"threshold":3,"text":"x3 HYPE","image":"./assets/hype.svg","sound":null,"variant":"fullscreen","duration_ms":5000},"user":{"id":"u3","name":"Cata","roles":["moderator"]}}
{"type":"interaction_combo","data":{"schema":1,"kind":"chat","rule":"chat-combo","key":"POGGERS","count":12,"threshold":5,"text":"x12 POGGERS","image":null,"sound":null,"variant":"combo","duration_ms":2500},"user":{"id":"u12","name":"Luz","roles":[]}}
{"type":"interaction_counter","data":{"schema":1,"rule":"muertes","name":"muertes","value":7,"text":"Muertes: 7","image":null,"sound":null,"variant":"counter","duration_ms":3500},"user":{"id":"mod1","name":"Mod","roles":["moderator"]}}
{"type":"interaction_emit","data":{"schema":1,"name":"camera_move","payload":{"preset":"desk"},"context":{"source":"command","rule":"camara","command":"camara","args":"desk","count":1,"platform":"twitch"}},"user":{"id":"mod1","name":"Mod","roles":["moderator"]}}
```

`source` es `command`, `mod_action`, `redemption` o `donation`; `step` empieza en 0.

### Patrón `camera_move`

Los nombres configurables no se convierten en tipos de evento: el host sólo permite emitir tipos declarados. Todos salen como `interaction_emit` con el nombre en `data.name`. Un plugin de cámara declara `listens = ["interaction_emit"]` y filtra:

```js
streams.on("interaction_emit", (data) => {
  if (data.name === "camera_move") moverCamara(data.payload.preset);
});
```

## Assets y seguridad

- `image` y `sound` son rutas relativas dentro de `overlay/assets` con letras ASCII, dígitos, `.`, `_`, `-` y `/`. Se rechazan rutas absolutas, `\`, `.`/`..`, componentes vacíos, archivos inexistentes, directorios y symlinks que escapan de la carpeta. Todo se valida al arrancar.
- El worker emite URLs `./assets/...`, nunca rutas locales. El overlay sólo asigna `img.src` o `new Audio` a URLs bajo `./assets/`.
- Los datos del chat se insertan con `textContent`; no se usa `innerHTML` ni estilos inline. `variant` sólo se convierte en clase tras validarse.
- Para añadir assets propios, cópialos a `overlay/assets/` y referencia su nombre en el TOML.

## Personalización CSS

Crea `styles/interactions.css` en el host; se sirve como `./custom.css` después de `style.css` y gana la cascada. Recarga el Browser Source.

Selectores:

- `.st-interactions`: raíz a pantalla completa.
- `.st-interactions__effect`: el efecto actual (`data-type` indica `interaction_show`, `interaction_combo` o `interaction_counter`).
- `.st-interactions__image`, `.st-interactions__text`, `.st-interactions__meta` (este último sólo visible cuando no hay texto).
- Variantes: `.st-interactions__effect--default`, `--fullscreen`, `--combo`, `--counter` y cualquier variant propia.
- Fases: `.st-interactions__effect--entering`, `--visible`, `--leaving`.
- Animaciones: `st-interactions-hype` (fullscreen) y `st-interactions-pulse` (combo). Con `prefers-reduced-motion` se desactivan.

| Variable | Variable |
|---|---|
| `--st-interactions-font-family` | `--st-interactions-color` |
| `--st-interactions-background` | `--st-interactions-accent` |
| `--st-interactions-width` | `--st-interactions-padding` |
| `--st-interactions-gap` | `--st-interactions-radius` |
| `--st-interactions-shadow` | `--st-interactions-text-size` |
| `--st-interactions-combo-size` | `--st-interactions-image-size` |
| `--st-interactions-enter-duration` | `--st-interactions-exit-duration` |
| `--st-interactions-enter-transform` | `--st-interactions-exit-transform` |

Ejemplo de `styles/interactions.css`:

```css
.st-interactions {
  --st-interactions-font-family: "Bebas Neue", sans-serif;
  --st-interactions-accent: #22d3ee;
  --st-interactions-combo-size: 7rem;
  --st-interactions-enter-duration: 400ms;
}

.st-interactions__effect--counter {
  left: 2rem;
  right: auto;
}

/* variant = "neon" en el TOML */
.st-interactions__effect--neon .st-interactions__text {
  color: #f0abfc;
  text-shadow: 0 0 1rem #f0abfc;
}
```

## Página de prueba

Abre `http://127.0.0.1:7777/p/interactions/test.html`. El iframe muestra el overlay real. Elige Twitch, Kick o YouTube y usa los once botones: `!hola mundo`, `!hi` de suscriptor, `!hype` de moderador, combo de tres moderadores, `!shoutout canal`, `!countdown 60`, combo POGGERS de cinco usuarios, `!muertes+`, `!muertes`, canje de Twitch y donación de YouTube. Cada botón publica eventos en `/api/events` y muestra cuántos aceptó el host. Los dos últimos necesitan `enabled = true` en sus `event_triggers` y reiniciar.

## Desarrollo

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
node --test
```

El worker es un bucle síncrono sobre stdin, sin Tokio ni hilos: toda expiración se evalúa al llegar un evento. stdout sólo transporta eventos JSONL; los diagnósticos van a stderr. Las pruebas usan un reloj manual y no duermen. No hay dependencias npm.

## Limitaciones

- No escribe en el chat (no hay `chat_write`).
- No hay recarga en caliente de la configuración principal ni de las reglas.
- No hace peticiones de red.
- No hay temporizadores persistentes ni cola persistente: la cola del overlay se pierde al recargar.
- Cooldowns, conteos `{count}` y combos viven en memoria y se reinician con el worker; sólo los contadores persisten.
