/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;
use test_driver_lib::run_cargo;

#[derive(Debug, StructOpt)]
pub struct CMakeCommand {
    #[structopt(long)]
    release: bool,
    #[structopt(long)]
    target: Option<String>,
    #[structopt(long)]
    verbose: bool,
    #[structopt(long)]
    prefix: Option<String>,
    #[structopt(long)]
    install: bool,
}

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or("cargo".into())
}

impl CMakeCommand {
    fn collect_native_libraries(
        &self,
        build_params: &[&str],
    ) -> Result<(Option<PathBuf>, Vec<PathBuf>), Box<dyn Error>> {
        use cargo_metadata::Message;

        let mut library_artifacts: Vec<PathBuf> = vec![];

        let mut target_dir = None;

        let mut params = vec!["-p", "sixtyfps-rendering-backend-default"];
        params.extend(build_params);

        let mut first_lib: PathBuf = PathBuf::new();

        run_cargo(&cargo(), "build", &params, |message| {
            match message {
                Message::CompilerArtifact(ref artifact) => {
                    if artifact.target.name != "sixtyfps-rendering-backend-default" {
                        return Ok(());
                    }

                    if let Some(native_lib_filename) =
                        artifact.filenames.iter().find_map(|filename| {
                            if let Some(ext) = filename.extension() {
                                if ext == "so" || ext == "dylib" || ext == "dll" {
                                    Some(filename)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                    {
                        let mut native_lib_dir = native_lib_filename.clone();
                        native_lib_dir.pop(); // strip off filename

                        if let Some(previously_found_target_dir) = &target_dir {
                            if native_lib_dir != *previously_found_target_dir {
                                return Err(format!("Unexpected artifact location found: Expected directory {:?} from {} but found artifact {} in {:?}", previously_found_target_dir, first_lib.display(), native_lib_filename.display(), native_lib_dir).into());
                            }
                        } else {
                            target_dir = Some(native_lib_dir.clone());
                            first_lib = native_lib_filename.clone();
                        }

                        library_artifacts.push(native_lib_filename.clone());
                    }
                }
                _ => (),
            }
            Ok(())
        })?;

        Ok((target_dir, library_artifacts))
    }

    pub fn build_cmake(&self) -> Result<(), Box<dyn Error>> {
        println!("CMake build");

        println!("Building libraries first");

        let mut params = vec![];
        if self.release {
            params.push("--release");
        }

        if let Some(target_triplet) = &self.target {
            params.push("--target");
            params.push(&target_triplet);
        }

        let (output_dir, library_artifacts) = self.collect_native_libraries(&params)?;

        if library_artifacts.is_empty() {
            return Err("Could not detect any native libraries in the build output".into());
        }

        let output_dir =
            output_dir.ok_or_else(|| "Failed to locate target directory from artifacts")?;

        println!("Generate header files");
        let mut include_dir = output_dir.clone();
        include_dir.push("include");
        test_driver_lib::cbindgen::gen_all(&include_dir)?;

        let mut libs_list = String::from("-DSIXTYFPS_INTERNAL_LIBS=");
        libs_list.push_str(
            &library_artifacts
                .iter()
                .map(|lib| lib.display().to_string())
                .collect::<Vec<String>>()
                .join(";"),
        );

        let source_dir = super::root_dir()?.join("api/sixtyfps-cpp/cmake");
        let binary_dir = output_dir;

        let mut cmd = std::process::Command::new("cmake");

        if self.verbose {
            cmd.arg("--trace-expand");
        }

        if let Some(prefix) = &self.prefix {
            let mut prefix_option = String::from("-DCMAKE_INSTALL_PREFIX=");
            prefix_option.push_str(prefix);
            cmd.arg(prefix_option);
        }

        let cmake_configure_status = cmd
            .arg(libs_list)
            .arg("-S")
            .arg(source_dir)
            .arg("-B")
            .arg(binary_dir.clone())
            .spawn()?
            .wait()?;

        if !cmake_configure_status.success() {
            return Err(format!(
                "CMake configure exited with code {:?}",
                cmake_configure_status.code()
            )
            .into());
        }

        let cmake_build_status = std::process::Command::new("cmake")
            .arg("--build")
            .arg(binary_dir.clone())
            .spawn()?
            .wait()?;
        if !cmake_build_status.success() {
            return Err(
                format!("CMake build exited with code {:?}", cmake_build_status.code()).into()
            );
        }

        if self.install {
            let cmake_install_status = std::process::Command::new("cmake")
                .arg("--install")
                .arg(binary_dir)
                .spawn()?
                .wait()?;
            if !cmake_install_status.success() {
                return Err(format!(
                    "CMake build exited with code {:?}",
                    cmake_install_status.code()
                )
                .into());
            }
        }

        Ok(())
    }
}
