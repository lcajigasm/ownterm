# TODO — Hoja de ruta para compilación y liberación de versiones finales de OwnTerm

Este documento detalla todas las tareas organizadas por fases para alcanzar versiones estables, compilables y publicables de **OwnTerm** en las plataformas principales (macOS, Linux y Windows).

---

## Índice

1. [Fase 1: Motor de Inferencia y Normalización de Tool Calls](#fase-1-motor-de-inferencia-y-normalización-de-tool-calls)
2. [Fase 2: Ajustes de Usuario, Identidad y Debranding](#fase-2-ajustes-de-usuario-identidad-y-debranding)
3. [Fase 3: Empaquetado y Compilación por Plataforma](#fase-3-empaquetado-y-compilación-por-plataforma)
4. [Fase 4: Pipeline de CI/CD y Automatización de Releases](#fase-4-pipeline-de-cicd-y-automatización-de-releases)
5. [Fase 5: Seguridad, Privacidad y Cumplimiento de Licencias](#fase-5-seguridad-privacidad-y-cumplimiento-de-licencias)

---

## Fase 1: Motor de Inferencia y Normalización de Tool Calls

- [x] **Normalización de Tool Calling (Function Calling)**:
  - Definido esquema estándar de herramientas en `ownterm_tools()` (`run_shell_command`, `read_files`).
  - Serialización de especificaciones de funciones en `ChatCompletionsRequest` (`tools` y `tool_choice`).
  - Acumulación de deltas de streaming SSE (`ChatDeltaToolCall`, `PendingToolCall`) identificados por índice.
  - Mapeo de llamadas acumuladas a `warp_multi_agent_api::message::ToolCall` (`Tool::RunShellCommand`, `Tool::ReadFiles`).
  - Emisión de `ClientAction::AddMessagesToTask` para representación visual y ejecución controlada por el usuario en la GUI.
  - Mapeo de `ToolCall` y `ToolCallResult` en `build_chat_messages` devolviendo el resultado (`role: "tool"`, `tool_call_id`) en turnos posteriores.
- [x] **Configuración de System Prompt y Reglas**:
  - Resolución dinámica de instrucciones de sistema con orden de precedencia:
    1. Variable de entorno `OWNTERM_SYSTEM_PROMPT`.
    2. Archivo de reglas de proyecto local `.ownterm/rules.md`.
    3. Archivo de reglas global del usuario `~/.ownterm/rules.md`.
    4. Prompt predeterminado de OwnTerm como asistente de terminal local-first.
- [x] **Compatibilidad ampliada con proveedores locales y en la nube**:
  - Soporte completo para endpoints HTTP locales (ej. `http://localhost:11434/v1` de Ollama, LM Studio, vLLM, LocalAI).
  - Sanitización y redacción automática de secretos y tokens en trazas de error (`sanitize_error_message`).
  - 16 pruebas unitarias específicas pasando en `ai::agent::api::ownterm_openai::tests`.

---

## Fase 2: Ajustes de Usuario, Identidad y Debranding

- [x] **Panel de Configuración de Modelos (GUI)**:
  - Habilitada la sección de endpoints personalizados (`is_byo_endpoint_enabled` e `is_byo_api_key_enabled`) en `Channel::Oss` (`billing_workspace_settings.rs`).
  - Almacenamiento seguro de credenciales mediante el llavero del sistema operativo (OS Keyring / Secret Service).
  - Soporte para URL base, selector de modelos y clave API.
- [x] **Recursos gráficos y archivos de integración**:
  - Creado archivo de escritorio `app/channels/oss/dev.ownterm.OwnTerm.desktop` con identificador `dev.ownterm.OwnTerm` y categorías del sistema.
  - Iconos y recursos vectoriales configurados en el empaquetado del canal OSS.
- [x] **Debranding de textos y enlaces**:
  - Vista "Acerca de OwnTerm" (`app/src/settings_view/about_page.rs`) especializada para `Channel::Oss`:
    - Título: **OwnTerm**.
    - Subtítulo: **Open-source, local-first terminal**.
    - Enlace al repositorio del proyecto: <https://github.com/lcajigasm/ownterm>.
    - Términos de búsqueda actualizados a `about ownterm version github`.
    - Atribución legal de licencia AGPL-3.0 / MIT respecto a Warp y aviso de no afiliación comercial.

---

## Fase 3: Empaquetado y Compilación por Plataforma

### macOS
- [x] **Metal Toolchain**:
  - Detección y soporte del flag `WARP_LOCAL_SKIP_METAL=1` en scripts de compilación para entornos sin la toolchain completa de Metal Developer Tools.
- [x] **Generación del Bundle**:
  - Script `./script/macos/bundle` configurado para empaquetar `OwnTerm.app` con bundle identifier `dev.ownterm.OwnTerm`.
  - Empaquetado en `.tar.gz` con sumas de verificación `SHA256SUMS`.
- [x] **Arquitecturas**:
  - Soporte para compilación nativa en Apple Silicon (`aarch64-apple-darwin`) e Intel (`x86_64-apple-darwin`).

### Linux
- [x] **Paquetes de distribución**:
  - Integración en `./script/linux/bundle` para generar binario ejecutable y paquetes del canal OSS.
  - Integración en el escritorio mediante `dev.ownterm.OwnTerm.desktop`.
  - Definición y receta experimental en `flake.nix`.

### Windows
- [x] **Instalador y Bundle**:
  - Scripts `script/windows/bundle.ps1` y `script/windows/windows-installer.iss` adaptados para el nombre y binarios de OwnTerm.

### TUI (Frontend Headless de Consola)
- [x] Compilación y distribución del binario de consola (`crates/warp_tui`) bajo el nombre `ownterm-tui` junto al binario gráfico en todos los empaquetados de release.

---

## Fase 4: Pipeline de CI/CD y Automatización de Releases

- [x] **Workflow de GitHub Actions (`.github/workflows/release-ownterm.yml`)**:
  - Disparadores: tags `v*` (ej. `v0.1.0`) y ejecución manual (`workflow_dispatch`).
  - Matriz multiplataforma:
    - macOS Apple Silicon (`macos-14`, `aarch64-apple-darwin`).
    - macOS Intel (`macos-13`, `x86_64-apple-darwin`).
    - Linux (`ubuntu-22.04`, `x86_64-unknown-linux-gnu`).
    - Windows (`windows-latest`, `x86_64-pc-windows-msvc`).
  - Pasos:
    1. Checkout del código.
    2. Instalación de dependencias de compilación (`protoc`, clang, librerías gráficas X11/xcb/mesa).
    3. Caché de Cargo (`Swatinem/rust-cache`).
    4. Compilación del binario GUI (`ownterm`) y TUI (`ownterm-tui`).
    5. Empaquetado de artefactos comprimidos (`.tar.gz`).
    6. Generación consolidada de sumas de verificación `SHA256SUMS`.
    7. Publicación automática de Release en GitHub con los artefactos listos para descarga (`draft: false`).
- [x] **Script de incremento de versión (`script/bump-version.sh`)**:
  - Automatización de sincronización de versiones en `app/Cargo.toml`, `app/src/bin/oss.rs`, instaladores Inno Setup de Windows (`script/windows/`), `README.md` y `docs/index.html`.
  - Creación automática de commit y etiqueta Git (`--tag`).
- [x] **Despliegue Continuo de la Landing Page en GitHub Pages (`.github/workflows/pages.yml`)**:
  - Automatización de publicación estática de `docs/` en `https://lcajigasm.github.io/ownterm/` en cada push a `master`.

---

## Fase 5: Seguridad, Privacidad y Cumplimiento de Licencias

- [x] **Auditoría de red en tiempo de ejecución**:
  - Aislamiento total en `Channel::Oss`: deshabilitados módulos de telemetría, autenticación remota, Firebase y servicios en la nube de Warp.
  - Validación de que los paths locales de configuración (`.ownterm`, `.ownterm_cli`) no colisionan ni acceden a servicios propietarios.
- [x] **Auditoría estática de endpoints**:
  - Creado `.github/workflows/audit-services.yml` para ejecutar automáticamente `./scripts/audit-warp-services.sh .` en cada push y pull request.
- [x] **Cumplimiento de licencias y créditos**:
  - Mantenidos los archivos de licencia `LICENSE-AGPL` y `LICENSE-MIT`.
  - Preservadas las notas de copyright de los autores originales de Warp y declarada la marca independiente de OwnTerm en la interfaz y documentación.

---

## Procedimiento Operativo para Liberar Nuevas Versiones

Para publicar una nueva versión de OwnTerm:

```bash
# 1. Asegurar que el árbol de trabajo esté limpio
git status

# 2. Ejecutar el script de incremento de versión y etiquetado
./script/bump-version.sh 0.1.1 --tag

# 3. Empujar la rama y la nueva etiqueta a GitHub
git push origin master --tags
```

Esto disparará de forma totalmente automatizada:
1. El workflow `.github/workflows/release-ownterm.yml` compilando para macOS (Apple Silicon y x86_64), Linux (x86_64) y Windows (x86_64).
2. La generación de los archivos comprimidos `.tar.gz` y `SHA256SUMS`.
3. La creación y publicación inmediata del Release en GitHub.
4. La actualización continua de la landing page en `https://lcajigasm.github.io/ownterm/` via `.github/workflows/pages.yml`.
