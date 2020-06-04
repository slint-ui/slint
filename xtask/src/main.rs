use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use structopt::StructOpt;

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
}

#[derive(Debug, StructOpt)]
pub enum TaskCommand {
    #[structopt(name = "cmake")]
    CMake(CMakeCommand),
}

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask")]
pub struct ApplicationArguments {
    #[structopt(subcommand)]
    pub command: TaskCommand,
}

fn root_dir() -> Result<PathBuf, Box<dyn Error>> {
    let mut root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").map_err(|_| "Cannot determine root directory - CARGO_MANIFEST_DIR is not set -- you can only run xtask via cargo")?);
    root.pop(); // $root/xtask -> $root
    Ok(root)
}

fn grep_for_prefixed_cargo_output<'a>(prefix: &str, lines: &'a str) -> Option<&'a str> {
    lines.lines().find_map(|line| {
        if line.starts_with(prefix) {
            Some(line[prefix.len()..].trim())
        } else {
            None
        }
    })
}

fn check_process_output(
    cmd: &Command,
    output: std::process::Output,
) -> Result<std::process::Output, Box<dyn Error>> {
    if !output.status.success() {
        Err(format!("Error running: {:?}\nstderr: {}\n", cmd, String::from_utf8(output.stderr)?)
            .into())
    } else {
        Ok(output)
    }
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or("cargo".into()))
}

#[derive(Debug)]
struct NativeLibrary {
    name: String,
    filename: PathBuf,
    native_library_dependencies: String,
}

impl CMakeCommand {
    fn collect_native_libraries(
        &self,
        build_params: &[&str],
    ) -> Result<(Option<PathBuf>, Vec<NativeLibrary>), Box<dyn Error>> {
        use cargo_metadata::Message;

        let mut result = vec![];

        let mut target_dir = None;

        let mut params = vec!["build", "--lib", "--message-format=json"];
        params.extend(build_params);

        let mut cmd = cargo().args(&params).stdout(std::process::Stdio::piped()).spawn().unwrap();

        let reader = std::io::BufReader::new(cmd.stdout.take().unwrap());

        for message in cargo_metadata::Message::parse_stream(reader) {
            match message.unwrap() {
                Message::CompilerArtifact(ref artifact) => {
                    if let Some(native_lib_filename) =
                        artifact.filenames.iter().find_map(|filename| {
                            if let Some(ext) = filename.extension() {
                                if ext == "a" || ext == "lib" {
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
                                return Err(format!("Unexpected artifact location found: Expected directory {:?} but found artifact in {:?}", previously_found_target_dir, native_lib_dir).into());
                            }
                        } else {
                            target_dir = Some(native_lib_dir.clone());
                        }

                        result.push(NativeLibrary {
                            name: artifact.target.name.clone(),
                            filename: native_lib_filename.clone(),
                            native_library_dependencies: self
                                .libraries_needed_for_runtime_linkage(&artifact.target.name)?,
                        });
                    }
                }
                _ => (),
            }
        }

        cmd.wait()?;

        Ok((target_dir, result))
    }

    fn libraries_needed_for_runtime_linkage(
        &self,
        package: &str,
    ) -> Result<String, Box<dyn Error>> {
        let mut cmd = cargo();
        cmd.args(&["rustc", "-p", package, "--", "--print=native-static-libs"]);
        let output = cmd.output()?;
        let output = check_process_output(&cmd, output)?;

        let native_libs_output = String::from_utf8(output.stderr).unwrap();

        let libraries =
            grep_for_prefixed_cargo_output("note: native-static-libs:", &native_libs_output)
                .ok_or_else(|| "Cannot find native-static-libs prefixed output")?;

        Ok(libraries.into())
    }

    fn build_cmake(&self) -> Result<(), Box<dyn Error>> {
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

        let (output_dir, native_libs) = self.collect_native_libraries(&params)?;

        if native_libs.is_empty() {
            return Err("Could not detect any native libraries in the build output".into());
        }

        let output_dir =
            output_dir.ok_or_else(|| "Failed to locate target directory from artifacts")?;

        let mut libs_list = String::from("-DSIXTYFPS_INTERNAL_LIBS=");
        libs_list.push_str(
            &native_libs
                .iter()
                .map(|lib| lib.filename.display().to_string())
                .collect::<Vec<String>>()
                .join(";"),
        );

        let mut external_libs_list = String::from("-DSIXTYFPS_EXTERNAL_LIBS=");
        external_libs_list.push_str(
            &native_libs
                .iter()
                .map(|lib| lib.native_library_dependencies.clone())
                .collect::<Vec<String>>()
                .join(" "),
        );

        let source_dir = root_dir()?.join("api/sixtyfps-cpp/cmake");
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
            .arg(external_libs_list)
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

        let cmake_build_status =
            std::process::Command::new("cmake").arg("--build").arg(binary_dir).spawn()?.wait()?;
        if !cmake_build_status.success() {
            return Err(
                format!("CMake build exited with code {:?}", cmake_build_status.code()).into()
            );
        }

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    match ApplicationArguments::from_args().command {
        TaskCommand::CMake(cmd) => cmd.build_cmake()?,
    };

    Ok(())
}
