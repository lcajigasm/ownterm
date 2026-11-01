# OwnTerm — matriz de huecos

OwnTerm es un fork del cliente Warp (Warp Services, Inc.). El núcleo del terminal sigue bajo AGPL-3.0 y MIT. OwnTerm no está afiliado a Warp. Esta matriz compara bloques del cliente upstream con el binario `ownterm` (`Channel::Oss`) en este repositorio.

La evidencia de compilación del cliente completo es el release [v0.4.0](https://github.com/lcajigasm/ownterm/releases/tag/v0.4.0), que adjunta archivos para macOS arm64, macOS x86_64, Linux x86_64 y Windows x86_64. El CI de pull request audita la privacidad, prueba el instalador sin red, cubre `ownterm_provider` y los tests de canal, y ejecuta `cargo check` de `ownterm` y `ownterm-tui` en Linux. El empaquetado de las cuatro plataformas sigue siendo el workflow del tag, que también adjunta un `.dmg` con `OwnTerm.app`.

## Matriz

| Pri. | Bloque | En Warp | En OwnTerm ahora | Hueco |
| :--- | :--- | :--- | :--- | :--- |
| P0 | Telemetría y crashes | RudderStack y Sentry en los canales de producto | Los macros no evalúan el evento si el canal no puede transmitir. `record_event` no construye ni encola nada salvo un opt-in de test. Sentry no arranca sin DSN | Quedan los tipos y los literales en el fuente. No se ejecutan en los binarios de OwnTerm |
| P0 | Cuenta y suscripción | Login obligatorio y créditos de IA | Sin proveedor, el chip muestra «No AI provider». En OSS, Ajustes abre en el agente y **Local provider** escribe `provider.env` (preset, modelo de la lista local, endpoint y el nombre `api_key_env`). La clave no se guarda | La descarga en vivo del catálogo `/models` sigue fuera del formulario |
| P0 | Proveedores de IA | Inferencia en los servidores de Warp | `Channel::Oss` usa `ownterm_openai` contra `/chat/completions`. Presets: Ollama, LM Studio, vLLM, LocalAI, OpenAI, xAI, OpenRouter, Groq y compatible. `suggested_models` y `parse_model_ids` cubren un catálogo estático y un documento `/models`. `ownterm-tui models` imprime sugerencias y la URL. El modelo puede llamar `run_shell_command`, `read_files`, `grep`, `file_glob` y `apply_file_diffs`; el runtime local los ejecuta | No hay adaptadores nativos de Anthropic ni Google. El comando no descarga el catálogo. El índice de código en la nube, el uso del ordenador y las herramientas de cuenta siguen fuera |
| P1 | Bloques, pestañas, paneles, temas | Sí | El código del cliente sigue en `app/` y se empaqueta en `ownterm` | No hay una pasada de verificación visual en esta tanda. Identidad visual todavía mezcla recursos upstream |
| P1 | Autocompletado y workflows | Sí, parte acoplada a Warp Drive y a la nube | La búsqueda de comandos incluye workflows de `~/.ownterm/workflows` sin Drive. El menú `/` inserta esos workflows. La guía de uso trae un YAML de ejemplo. El palette busca archivos locales | Los workflows de agente en la nube y el catálogo de Drive siguen apagados. No hay editor visual para crear el YAML |
| P1 | Warp Drive, equipos, billing, Oz | Núcleo del producto de pago | Servidor, RTC y Oz apuntan a `127.0.0.1:9`. Flags de cloud, billing y onboarding de cuenta están apagados en OSS. La barra de Ajustes en OSS omite Cuenta, Billing, Teams, Warp Drive, Referrals, bloques compartidos, entornos cloud, claves de API cloud y Warpify | No hay sustituto local de sincronización. El código de esas pantallas sigue en el binario |
| P2 | Actualizaciones automáticas | `releases.warp.dev` | `script/install-ownterm.sh` baja el archivo de GitHub Releases, comprueba `SHA256SUMS` y sustituye los binarios. `autoupdate_config` sigue vacío, así que la app no descarga nada | No hay updater dentro de la app |
| P2 | Firma y notarización | Firmado por Warp | Los artefactos de Actions no se firman. La guía de uso avisa de Gatekeeper y SmartScreen | macOS Gatekeeper y SmartScreen quedan para quien instala el tarball |
| P3 | CI del cliente completo | Interno | El PR ejecuta `cargo check` de `ownterm` y `ownterm-tui` en Linux, además de la auditoría y `ownterm_provider` | No sustituye el build de release de las cuatro plataformas |

## Telemetría retirada o desactivada en esta tanda

- Envío de lotes a RudderStack (`may_transmit_telemetry`): el canal OSS nunca transmite; sin config de telemetría tampoco.
- Arranque del colector de red, volcado al cerrar y reenvío del spool en disco. En OSS el spool se borra y la cola en memoria se vacía cada 30 s.
- Inicialización de Sentry cuando no hay DSN (`is_crash_reporting_available`).
- `telemetry_config`, `crash_reporting_config`, autoupdate y endpoints de Warp en los binarios de canal, vía `ChannelConfig::without_warp_services`.
- Clave de cliente Firebase que estaba en `WarpServerConfig::production`.
- Interruptor de telemetría de sandbox (`WITH_SANDBOX_TELEMETRY`) y `ForceLogin` en el binario preview.
- Preferencias en memoria de telemetría, crash reports y almacenamiento de conversaciones en la nube cuando el canal no puede transmitir.

## Siguientes tres huecos

1. Pasada visual de bloques, pestañas, paneles y temas: la identidad sigue mezclando recursos de Warp.
2. Firma y notarización de los binarios. El instalador comprueba `SHA256SUMS`; Gatekeeper y SmartScreen siguen avisando.
3. Descarga en vivo del catálogo `/models` del endpoint del usuario. El formulario ya ofrece la lista local.
