#!/usr/bin/env bash
# Obsolete path. MDE look/theme is look_mde.sh (sourced by mde/install.sh).
# Kept so accidental `source look_sway.sh` still loads the canonical look.
# shellcheck source=look_mde.sh
source "$(dirname "${BASH_SOURCE[0]}")/look_mde.sh"
