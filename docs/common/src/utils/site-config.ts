// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export const BASE_URL = "https://localhost";
/** Base path of the main Slint documentation Starlight site (`docs/astro`) on slint.dev. */
export const BASE_PATH = "/docs/";
/**
 * Base path of the Material components documentation site (`ui-libraries/material/docs`).
 * That app is served at the domain root (see `src/config.yaml` `site.base`).
 */
export const MATERIAL_DOCS_BASE_PATH = "/";
export const SLINT_DOWNLOAD_VERSION = "nightly";
export const CPP_BASE_URL = `${BASE_PATH}../cpp/`;
export const RUST_BASE_URL = `${BASE_PATH}../rust/`;
export const RUST_SLINT_CRATE_URL = `${RUST_BASE_URL}slint/`;
export const RUST_SLINT_INTERPRETER_CRATE_URL = `${RUST_BASE_URL}slint_interpreter/`;
export const RUST_SLINT_BUILD_CRATE_URL = `${RUST_BASE_URL}slint_build/`;
export const NODEJS_BASE_URL = `${BASE_PATH}../node/`;
export const PYTHON_BASE_URL = `${BASE_PATH}../python/`;
