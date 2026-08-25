#!/usr/bin/env bash
# Shared repository-boundary reader for launch scripts.

if ! declare -F vyre_read_toml_values >/dev/null 2>&1; then
    source scripts/lib/toml_reader.sh
fi

vyre_load_repo_boundary() {
    local manifest="${VYRE_REPO_BOUNDARY_MANIFEST:-release/repo-boundary.toml}"
    if ! vyre_read_toml_values \
        "$manifest" \
        "repository-boundary" \
        4 \
        "public_repository" \
        "private_repository" \
        "verify_public_repo_action" \
        "boundary_description"; then
        return 2
    fi
    VYRE_RELEASE_PUBLIC_REPO="${VYRE_TOML_VALUES[0]}"
    VYRE_RELEASE_PRIVATE_REPO="${VYRE_TOML_VALUES[1]}"
    VYRE_RELEASE_VERIFY_PUBLIC_REPO_ACTION="${VYRE_TOML_VALUES[2]}"
    VYRE_RELEASE_REPO_BOUNDARY_DESCRIPTION="${VYRE_TOML_VALUES[3]}"
}
