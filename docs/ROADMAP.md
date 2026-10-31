# OwnTerm — matriz de huecos

OwnTerm es un fork del cliente Warp (Warp Services, Inc.). El núcleo del terminal sigue bajo AGPL-3.0 y MIT. OwnTerm no está afiliado a Warp. Esta matriz compara bloques del cliente upstream con el binario `ownterm` (`Channel::Oss`) en este repositorio.

La evidencia de compilación del cliente completo es el release [v0.1.1](https://github.com/lcajigasm/ownterm/releases/tag/v0.1.1), que adjunta archivos para macOS arm64, macOS x86_64, Linux x86_64 y Windows x86_64. Esta tanda no recompiló ese binario en local. El CI de pull request cubre la auditoría de privacidad, `ownterm_provider` y los tests de configuración de canal.

## Matriz

| Pri. | Bloque | En Warp | En OwnTerm ahora | Hueco |
| :--- | :--- | :--- | :--- | :--- |
| P0 | Telemetría y crashes | RudderStack y Sentry en los canales de producto | Los macros no evalúan el evento si el canal no puede transmitir. `record_event` no construye ni encola nada salvo un opt-in de test. Sentry no arranca sin DSN | Quedan los tipos y los literales en el fuente. No se ejecutan en los binarios de OwnTerm |
| P0 | Cuenta y suscripción | Login obligatorio y créditos de IA | Sin proveedor, el chip y Ajustes muestran «No AI provider» y abren la configuración local. Con proveedor directo no hay aviso de créditos | No hay un asistente gráfico para escribir `provider.env` |
| P0 | Proveedores de IA | Inferencia en los servidores de Warp | `Channel::Oss` usa `ownterm_openai` contra `/chat/completions`. Presets: Ollama, LM Studio, vLLM, LocalAI, OpenAI, xAI, OpenRouter, Groq y compatible. Herramientas `run_shell_command` y `read_files` | No hay adaptadores nativos de Anthropic ni Google. El catálogo de modelos no se descarga del proveedor. Faltan más herramientas del agente Warp |
| P1 | Bloques, pestañas, paneles, temas | Sí | El código del cliente sigue en `app/` y se empaqueta en `ownterm` | No hay una pasada de verificación visual en esta tanda. Identidad visual todavía mezcla recursos upstream |
| P1 | Autocompletado y workflows | Sí, parte acoplada a Warp Drive y a la nube | La búsqueda de comandos incluye workflows de `~/.ownterm/workflows` sin Drive. El menú `/` inserta esos workflows. El palette busca archivos locales (`CommandPaletteFileSearch`). El autocompletado de shell no llama a Warp | Los workflows de agente en la nube y el catálogo de Drive siguen apagados. No hay editor visual para crear el YAML |
| P1 | Warp Drive, equipos, billing, Oz | Núcleo del producto de pago | Servidor, RTC y Oz apuntan a `127.0.0.1:9`. Flags de cloud, billing y onboarding de cuenta están apagados en OSS | No hay sustituto local de sincronización. Esas pantallas pueden seguir existiendo y no hacer nada útil |
| P2 | Actualizaciones automáticas | `releases.warp.dev` | `autoupdate_config` vacío en OSS y tras `without_warp_services()` | Las releases salen por GitHub. No hay updater dentro de la app |
| P2 | Firma y notarización | Firmado por Warp | Los artefactos de Actions no se firman | macOS Gatekeeper y SmartScreen quedan para quien instala el tarball |
| P3 | CI del cliente completo | Interno | El PR solo compila el crate de proveedores y `warp_core` (tests de canal). El binario completo se construye al etiquetar `vX.Y.Z` | Un fallo de compilación del GUI no se ve hasta el workflow de release |

## Telemetría retirada o desactivada en esta tanda

- Envío de lotes a RudderStack (`may_transmit_telemetry`): el canal OSS nunca transmite; sin config de telemetría tampoco.
- Arranque del colector de red, volcado al cerrar y reenvío del spool en disco. En OSS el spool se borra y la cola en memoria se vacía cada 30 s.
- Inicialización de Sentry cuando no hay DSN (`is_crash_reporting_available`).
- `telemetry_config`, `crash_reporting_config`, autoupdate y endpoints de Warp en los binarios de canal, vía `ChannelConfig::without_warp_services`.
- Clave de cliente Firebase que estaba en `WarpServerConfig::production`.
- Interruptor de telemetría de sandbox (`WITH_SANDBOX_TELEMETRY`) y `ForceLogin` en el binario preview.
- Preferencias en memoria de telemetría, crash reports y almacenamiento de conversaciones en la nube cuando el canal no puede transmitir.

## Siguientes tres huecos

1. Asistente en Ajustes para escribir el proveedor (preset, modelo y endpoint) sin editar `provider.env` a mano, y un catálogo de modelos del endpoint.
2. Pasada visual de bloques, pestañas, paneles y temas: la identidad sigue mezclando recursos de Warp.
3. Actualizador desde las releases de GitHub, sin `releases.warp.dev`, y firma de los binarios.
