# OwnTerm — matriz de huecos

OwnTerm es un fork del cliente Warp (Warp Services, Inc.). El núcleo del terminal sigue bajo AGPL-3.0 y MIT. OwnTerm no está afiliado a Warp. Esta matriz compara bloques del cliente upstream con el binario `ownterm` (`Channel::Oss`) en este repositorio.

La evidencia de compilación del cliente completo es el release [v0.1.1](https://github.com/lcajigasm/ownterm/releases/tag/v0.1.1), que adjunta archivos para macOS arm64, macOS x86_64, Linux x86_64 y Windows x86_64. Esta tanda no recompiló ese binario en local. El CI de pull request cubre la auditoría de privacidad, `ownterm_provider` y los tests de configuración de canal.

## Matriz

| Pri. | Bloque | En Warp | En OwnTerm ahora | Hueco |
| :--- | :--- | :--- | :--- | :--- |
| P0 | Telemetría y crashes | RudderStack y Sentry en los canales de producto | El canal OSS no transmite. Los binarios `stable`/`preview`/`dev`/`local` llaman `without_warp_services()`. La clave de Firebase embebida se vació. Sentry no arranca sin DSN. La cola en memoria se descarta y los ficheros de spool se borran sin subirlos | Siguen existiendo cientos de constructores de eventos. No salen de la máquina, pero el código de analítica sigue en el árbol |
| P0 | Cuenta y suscripción | Login obligatorio y créditos de IA | OSS entra al terminal sin login. Con `OWNTERM_PROVIDER`, `OWNTERM_BASE_URL`, `~/.ownterm/provider.env` o un endpoint custom, el agente no aplica la cuota Warp ni el aviso de créditos. Ajustes ya no pide crear cuenta en OSS y oculta el fallback de créditos Warp | Sin proveedor configurado, la UI heredada de créditos puede seguir apareciendo. No sustituye al plan de Warp; solo deja de bloquear cuando hay un endpoint propio |
| P0 | Proveedores de IA | Inferencia en los servidores de Warp | `Channel::Oss` usa `ownterm_openai` contra `/chat/completions`. Presets: Ollama, LM Studio, vLLM, LocalAI, OpenAI, xAI, OpenRouter, Groq y compatible. Herramientas `run_shell_command` y `read_files` | No hay adaptadores nativos de Anthropic ni Google. El catálogo de modelos no se descarga del proveedor. Faltan más herramientas del agente Warp |
| P1 | Bloques, pestañas, paneles, temas | Sí | El código del cliente sigue en `app/` y se empaqueta en `ownterm` | No hay una pasada de verificación visual en esta tanda. Identidad visual todavía mezcla recursos upstream |
| P1 | Autocompletado y workflows | Sí, parte acoplada a Warp Drive y a la nube | El código está. OSS desactiva `AgentModeWorkflows` y los flags de Drive/cloud | Hace falta un camino local que no consulte objetos de Drive ni servidores Warp |
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

1. Sustituir los avisos heredados de créditos y de “crear cuenta” cuando no hay proveedor, por un estado vacío que lleve a la configuración local.
2. Dejar de construir eventos de analítica en el árbol, no solo de tirarlos antes de la red.
3. Workflows y autocompletado que funcionen sin Warp Drive ni servidores de Warp.
