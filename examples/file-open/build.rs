// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cSpell: ignore APPL slintsave

use std::io::Write as _;

fn main() {
    slint_build::compile("ui/main.slint").unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let mut plist = std::fs::File::create(out_dir.join("Info.plist")).unwrap();
        plist.write_all(INFO_PLIST.as_bytes()).unwrap();
    }
}

const INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>file-open</string>
	<key>CFBundleDisplayName</key>
	<string>Slint Backup Inspector</string>
	<key>CFBundleIdentifier</key>
	<string>dev.slint.backup-inspector</string>
	<key>CFBundleExecutable</key>
	<string>file-open</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>1.0</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>CFBundleDocumentTypes</key>
	<array>
		<dict>
			<key>CFBundleTypeName</key>
			<string>Slint Backup Archive</string>
			<key>CFBundleTypeRole</key>
			<string>Viewer</string>
			<key>LSHandlerRank</key>
			<string>Owner</string>
			<key>LSItemContentTypes</key>
			<array>
				<string>dev.slint.backup-archive</string>
			</array>
		</dict>
	</array>
	<key>UTExportedTypeDeclarations</key>
	<array>
		<dict>
			<key>UTTypeIdentifier</key>
			<string>dev.slint.backup-archive</string>
			<key>UTTypeDescription</key>
			<string>Slint Backup Archive</string>
			<key>UTTypeConformsTo</key>
			<array>
				<string>public.data</string>
			</array>
			<key>UTTypeTagSpecification</key>
			<dict>
				<key>public.filename-extension</key>
				<array>
					<string>slintsave</string>
				</array>
				<key>public.mime-type</key>
				<array>
					<string>application/x-slint-backup</string>
				</array>
			</dict>
		</dict>
	</array>
</dict>
</plist>
"#;
