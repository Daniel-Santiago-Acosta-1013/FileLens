#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY_NAME="filelens"
TARGET="${PROJECT_ROOT}/target/release/${BINARY_NAME}"
PREFIX="${PREFIX:-/usr/local}"
BINDIR="${BINDIR:-${PREFIX}/bin}"

print_step() {
  printf '\033[1;34m==>\033[0m %s\n' "$1"
}

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: cargo no está instalado. Instala Rust desde https://www.rust-lang.org/ antes de continuar." >&2
  exit 1
fi

print_step "Compilando ${BINARY_NAME} en modo release"
(cd "$PROJECT_ROOT" && cargo build --release)

if [ ! -f "$TARGET" ]; then
  echo "Error: no se generó el binario esperado en $TARGET" >&2
  exit 1
fi

print_step "Creando directorio de destino ${BINDIR}"
mkdir -p "$BINDIR"

print_step "Instalando binario en ${BINDIR}/${BINARY_NAME}"
install -m 755 "$TARGET" "${BINDIR}/${BINARY_NAME}"

cat <<MSG
${BINARY_NAME} instalado correctamente en ${BINDIR}/${BINARY_NAME}
Asegúrate de que ${BINDIR} esté en tu PATH para poder ejecutar ${BINARY_NAME} desde cualquier directorio.

Puedes sobreescribir la ruta de instalación ejecutando, por ejemplo:
  PREFIX=$HOME/.local ./scripts/install.sh
MSG
