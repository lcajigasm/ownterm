# OwnTerm — matriz de huecos

OwnTerm es un fork del cliente Warp (Warp Services, Inc.). El núcleo del terminal sigue bajo AGPL-3.0 y MIT. OwnTerm no está afiliado a Warp. Esta matriz compara bloques del cliente upstream con el binario `ownterm` (`Channel::Oss`) en este repositorio. Dónde vive cada mecanismo está en [ARCHITECTURE.md](ARCHITECTURE.md).

El CI de pull request audita la privacidad, prueba el instalador sin red, cubre `ownterm_provider`, ejecuta `cargo check` de `ownterm` y `ownterm-tui` en Linux y corre los tests del agente local, de la política de flags y del arranque del TUI. OwnTerm solo se publica para macOS (arm64 y x86_64); el empaquetado es el workflow del tag. El código de Linux y Windows heredado de Warp sigue en el árbol, pero no se compila, prueba ni publica.

## Matriz

| Pri. | Bloque | En OwnTerm ahora | Hueco |
| :--- | :--- | :--- | :--- |
| P0 | Agente local | `ownterm_openai` hace el papel del servicio de agentes: crea la tarea, registra la consulta y los resultados de herramientas, y reconstruye el historial en cada petición. El bucle con herramientas se ha probado con tests contra una conversación real del cliente y ejecutando `ownterm-tui` contra un endpoint simulado | No se ha probado contra cada proveedor real. Herramientas: `run_shell_command`, `read_files`, `grep`, `file_glob`, `apply_file_diffs`. Los servidores MCP locales no se ofrecen al modelo. Sin búsqueda web, subagentes ni comandos interactivos de larga duración |
| P0 | Proveedores de IA | Cualquier endpoint compatible con `/chat/completions`. Presets: Ollama, LM Studio, vLLM, LocalAI, OpenAI, xAI, OpenRouter, Groq y compatible. `model=auto` elige el modelo más barato disponible y respeta el `base_url` configurado | No hay adaptadores nativos de Anthropic ni Google. No se descarga `/models` en vivo |
| P0 | Clave de API | Solo desde una variable de entorno; nunca se escribe en disco | La app de escritorio lanzada desde Finder o el Dock no hereda el entorno del shell, así que ahí un proveedor en la nube solo funciona con el endpoint personalizado de Ajustes. Las dos vías de configuración (`provider.env` y Ajustes) no están unificadas |
| P0 | Telemetría y crashes | Los macros no evalúan el evento si el canal no puede transmitir. Sentry no arranca sin DSN | Quedan los tipos y los literales en el fuente. No hay un test de CI que arranque el binario con la red denegada |
| P1 | Cuenta, Drive, equipos, billing | Servidor, RTC y Oz apuntan a `127.0.0.1:9`. Los flags de nube, compartir sesión, actualizador, changelog e indexado remoto están apagados. Los menús, la paleta y los Ajustes omiten cuenta, Drive, equipos, referidos, feedback y Slack. El modal de login no se abre | El código de esas pantallas sigue en el binario. Compartir bloque sigue visible en el menú contextual del bloque. La lista de conversaciones del panel izquierdo sigue apagada |
| P1 | IA respaldada por servidor | Sugerencia de siguiente comando, sugerencias de prompt, búsqueda de comandos con IA, mensajes de commit, títulos de bloque y voz están apagados porque llamaban al servidor de Warp. Renombrar una conversación es local | Sus interruptores siguen visibles en Ajustes aunque no hacen nada. Ninguna de esas funciones se ha reimplementado sobre el modelo local |
| P1 | Marca | Título de ventana, menús, Ajustes, textos del agente, TUI, primeros pasos y enlaces de ayuda dicen OwnTerm y apuntan a este repositorio | Quedan unos 300 enlaces «Learn more» a `docs.warp.dev`, el texto `--help` de `crates/warp_cli`, y los nombres `TERM_PROGRAM=WarpTerminal` y `WARP_*`, que se mantienen por compatibilidad |
| P2 | Actualizaciones | `script/install-ownterm.sh` baja el archivo de GitHub Releases y comprueba `SHA256SUMS`. La app no descarga nada | No hay updater dentro de la app |
| P3 | Linux y Windows | Aparcados. No hay binarios ni CI de release para ellos | Volver a publicar exige recuperar sus jobs de build. `.deb` y `.rpm` siguen bloqueados: sus scripts registran el repositorio y la clave de Warp |
| P2 | Licencias de terceros | Los archivos incluyen `LICENSE-AGPL` y `LICENSE-MIT`. «Acerca de» enlaza el repositorio | Los archivos no incluyen `THIRD_PARTY_LICENSES` |
| P3 | Firma y notarización | Aplazada. La guía de uso avisa de Gatekeeper | Los artefactos no se firman |

## Siguientes tres huecos

1. Una forma de dar la clave de API a la app de escritorio sin entorno de shell, y unificar `provider.env` con el endpoint de Ajustes.
2. Ofrecer al modelo los servidores MCP locales y descargar `/models` del endpoint del usuario.
3. Test de CI con la red denegada, e incluir `THIRD_PARTY_LICENSES` en los archivos.
