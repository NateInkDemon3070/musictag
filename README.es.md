# musictag

Editor de metadatos de musica escrito en Rust con TUI, disenado para archivos de audio de cualquier formato: FLAC, OPUS, M4A y mas! Usa Vim keys.

## Caracteristicas

- Navega directorios y edita metadatos uno por uno
- Edicion masiva: selecciona varios archivos y editalos todos a la vez
- Elimina archivos directamente desde la app
- Establece una carpeta predeterminada que se abre al iniciar
- Detecta automaticamente el idioma del sistema (Espanol/Ingles)
- Soporta MP3, FLAC, OGG, Opus, M4A/AAC, WAV, APE, WMA
- Iconos de Nerd Font

## Atajos de teclado

### Modo Navegacion

| Tecla | Accion |
|-------|--------|
| `j` / `↓` | Mover abajo |
| `k` / `↑` | Mover arriba |
| `h` / `←` | Volver al directorio padre |
| `l` / `→` / `Enter` | Abrir carpeta o editar archivo |
| `Esc` | Volver al directorio padre |
| `g` / `Home` | Ir al primer elemento |
| `G` / `End` | Ir al ultimo elemento |
| `a` / `Espacio` | Seleccionar/deseleccionar |
| `v` | Seleccionar todos los archivos de audio |
| `A` | Editar todos los seleccionados (masivo) |
| `d` | Limpiar seleccion |
| `f` / `/` | Filtrar archivos |
| `r` | Recargar directorio |
| `C` | Guardar carpeta como predeterminada |
| `x` / `Delete` | Eliminar archivo(s) |
| `q` | Salir |

### Modo Edicion

| Tecla | Accion |
|-------|--------|
| `Tab` / `Shift+Tab` | Siguiente / anterior campo |
| `↑` / `↓` | Siguiente / anterior campo |
| `←` / `→` | Mover cursor en el texto |
| `Home` / `End` | Inicio / final del texto |
| `Backspace` | Borrar caracter anterior |
| `Delete` | Borrar caracter actual |
| `Ctrl+U` | Limpiar campo actual |
| `Enter` | Guardar y volver a navegacion |
| `Ctrl+S` | Guardar y editar siguiente archivo |
| `Ctrl+C` | Aplicar campo actual a todos los seleccionados |
| `Esc` | Cancelar sin guardar |

### Modo Filtro

| Tecla | Accion |
|-------|--------|
| Escribir | Filtrado en tiempo real |
| `Enter` | Aplicar filtro |
| `Backspace` | Borrar caracter |
| `Ctrl+U` | Limpiar filtro |
| `Esc` | Cancelar filtro |

## Instalar

```bash
# Desde fuente (requiere Rust)
git clone https://github.com/youruser/musictag.git
cd musictag
chmod +x install.sh
./install.sh
```

O con makepkg en Arch/Artix:

```bash
makepkg -si
```

## Licencia

MIT
