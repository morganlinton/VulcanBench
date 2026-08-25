#!/usr/bin/env bash
# Shared release train reader for launch and publish scripts.

if ! declare -F vyre_read_toml_values >/dev/null 2>&1; then
    source scripts/lib/toml_reader.sh
fi

vyre_load_release_train() {
    local manifest="${VYRE_RELEASE_TRAIN_MANIFEST:-release/release-train.toml}"
    if ! vyre_read_toml_values \
        "$manifest" \
        "release-train" \
        5 \
        "versions.vyre" \
        "versions.weir" \
        "tags.vyre" \
        "tags.weir" \
        "tags.combined_release_train"; then
        return 2
    fi
    VYRE_RELEASE_VYRE_VERSION="${VYRE_TOML_VALUES[0]}"
    VYRE_RELEASE_WEIR_VERSION="${VYRE_TOML_VALUES[1]}"
    VYRE_RELEASE_TAG_VYRE="${VYRE_TOML_VALUES[2]}"
    VYRE_RELEASE_TAG_WEIR="${VYRE_TOML_VALUES[3]}"
    VYRE_RELEASE_TAG_COMBINED="${VYRE_TOML_VALUES[4]}"
    VYRE_RELEASE_LAUNCH_APPROVAL_TOKEN="launch-vyre-${VYRE_RELEASE_VYRE_VERSION}-weir-${VYRE_RELEASE_WEIR_VERSION}"
    VYRE_RELEASE_PUBLISH_APPROVAL_TOKEN="publish-vyre-${VYRE_RELEASE_VYRE_VERSION}-weir-${VYRE_RELEASE_WEIR_VERSION}"
    VYRE_RELEASE_DISPLAY="Vyre ${VYRE_RELEASE_VYRE_VERSION} / Weir ${VYRE_RELEASE_WEIR_VERSION}"
    VYRE_RELEASE_TAGS=(
        "$VYRE_RELEASE_TAG_VYRE"
        "$VYRE_RELEASE_TAG_WEIR"
        "$VYRE_RELEASE_TAG_COMBINED"
    )
}
