// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package main

import (
	"bufio"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
)

type buildConfig struct {
	goos        string
	goarch      string
	targetID    string
	rustTarget  string
	profile     string
	outputFile  string
	outputDir   string
	buildRoot   string
	distDir     string
	objDir      string
	artifactDir string
}

func repoRoot() string {
	_, file, _, _ := runtime.Caller(0)
	return filepath.Clean(filepath.Join(filepath.Dir(file), "../../../.."))
}

func run(dir string, env []string, name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	cmd.Env = env
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	return cmd.Run()
}

func envOrDefault(name string, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}

func rustHostTarget(root string) (string, error) {
	cmd := exec.Command("rustc", "-vV")
	cmd.Dir = root
	output, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("failed to query rust host target: %w", err)
	}
	for line := range strings.Lines(string(output)) {
		if value, ok := strings.CutPrefix(strings.TrimSpace(line), "host: "); ok {
			// replace x86_64-pc-windows-msvc into x86_64-pc-windows-gnu
			value = strings.ReplaceAll(value, "-msvc", "-gnu")
			return value, nil
		}
	}
	return "", fmt.Errorf("failed to parse rust host target")
}

func rustTargetForGo(root string, goos string, goarch string) (string, error) {
	if target := os.Getenv("SLINT_RUST_TARGET"); target != "" {
		return target, nil
	}
	if goos == runtime.GOOS && goarch == runtime.GOARCH {
		return rustHostTarget(root)
	}

	target := map[[2]string]string{
		{"linux", "amd64"}:   "x86_64-unknown-linux-gnu",
		{"linux", "arm64"}:   "aarch64-unknown-linux-gnu",
		{"darwin", "amd64"}:  "x86_64-apple-darwin",
		{"darwin", "arm64"}:  "aarch64-apple-darwin",
		{"windows", "amd64"}: "x86_64-pc-windows-gnu",
	}[[2]string{goos, goarch}]
	if target == "" {
		return "", fmt.Errorf(
			"unsupported target %s/%s; set SLINT_RUST_TARGET to a Rust target triple",
			goos,
			goarch,
		)
	}
	return target, nil
}

func buildProfile() string {
	profile := envOrDefault("SLINT_BUILD_PROFILE", "release")
	if profile == "debug" {
		return profile
	}
	return "release"
}

func targetGoOS() string {
	return envOrDefault("SLINT_GOOS", runtime.GOOS)
}

func targetGoArch() string {
	return envOrDefault("SLINT_GOARCH", runtime.GOARCH)
}

func newBuildConfig(root string, outputFile string) (*buildConfig, error) {
	goos := targetGoOS()
	goarch := targetGoArch()
	rustTarget, err := rustTargetForGo(root, goos, goarch)
	if err != nil {
		return nil, err
	}
	outputDir := filepath.Dir(outputFile)
	targetID := goos + "_" + goarch
	buildRoot := filepath.Join(outputDir, ".slint-build")
	objDir := filepath.Join(buildRoot, "obj", targetID)
	distDir := filepath.Join(buildRoot, "dist", targetID)
	profile := buildProfile()
	return &buildConfig{
		goos:        goos,
		goarch:      goarch,
		targetID:    targetID,
		rustTarget:  rustTarget,
		profile:     profile,
		outputFile:  outputFile,
		outputDir:   outputDir,
		buildRoot:   buildRoot,
		distDir:     distDir,
		objDir:      objDir,
		artifactDir: filepath.Join(objDir, rustTarget, profile),
	}, nil
}

func buildEnv(config *buildConfig) []string {
	env := slices.Clone(os.Environ())
	env = append(env, "CARGO_TARGET_DIR="+config.objDir)
	return env
}

func cargoArgs(config *buildConfig, packageName string) []string {
	args := []string{"build", "-p", packageName, "--target", config.rustTarget}
	if packageName == "slint-cpp" {
		args = append(args, "--features", "interpreter")
	}
	if config.profile == "release" {
		args = append(args, "--release")
	}
	return args
}

func compilerArgs(config *buildConfig, output string, input string) []string {
	args := []string{"run", "-p", "slint-compiler"}
	if config.profile == "release" {
		args = append(args, "--release")
	}
	args = append(args, "--", "-f", "go", "-o", output, input)
	return args
}

func ensureBuildLayout(config *buildConfig) error {
	for _, dir := range []string{
		config.outputDir,
		config.distDir,
		config.objDir,
	} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return err
		}
	}
	if err := os.MkdirAll(config.buildRoot, 0o755); err != nil {
		return err
	}
	gitignorePath := filepath.Join(config.buildRoot, ".gitignore")
	return os.WriteFile(gitignorePath, []byte("/obj/\n"), 0o644)
}

func copyFile(src string, dst string) error {
	input, err := os.Open(src)
	if err != nil {
		return err
	}
	defer input.Close()

	output, err := os.Create(dst)
	if err != nil {
		return err
	}

	if _, err := io.Copy(output, input); err != nil {
		output.Close()
		return err
	}
	return output.Close()
}

func matchingArtifacts(config *buildConfig) ([]string, error) {
	entries, err := os.ReadDir(config.artifactDir)
	if err != nil {
		return nil, err
	}
	artifacts := make([]string, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		name := entry.Name()
		if strings.Contains(name, "slint_cpp") {
			artifacts = append(artifacts, filepath.Join(config.artifactDir, name))
		}
	}
	return artifacts, nil
}

func sharedArtifactNames(goos string) []string {
	switch goos {
	case "darwin":
		return []string{"libslint_cpp.dylib"}
	case "windows":
		return []string{"slint_cpp.dll", "slint_cpp.dll.a", "slint_cpp.lib"}
	default:
		return []string{"libslint_cpp.so"}
	}
}

func staticArtifactNames(goos string) []string {
	if goos == "windows" {
		return []string{"slint_cpp.lib", "libslint_cpp.a"}
	}
	return []string{"libslint_cpp.a"}
}

func stageArtifacts(config *buildConfig) (string, []string, error) {
	if err := os.RemoveAll(config.distDir); err != nil {
		return "", nil, err
	}
	if err := os.MkdirAll(config.distDir, 0o755); err != nil {
		return "", nil, err
	}

	artifacts, err := matchingArtifacts(config)
	if err != nil {
		return "", nil, err
	}

	var staticName string
	var copied []string
	copyNamed := func(name string) error {
		for _, artifact := range artifacts {
			if filepath.Base(artifact) != name {
				continue
			}
			dst := filepath.Join(config.distDir, name)
			if err := copyFile(artifact, dst); err != nil {
				return err
			}
			copied = append(copied, name)
			return nil
		}
		return nil
	}

	for _, name := range sharedArtifactNames(config.goos) {
		if err := copyNamed(name); err != nil {
			return "", nil, err
		}
	}
	for _, name := range staticArtifactNames(config.goos) {
		if err := copyNamed(name); err != nil {
			return "", nil, err
		}
		if slices.Contains(copied, name) && staticName == "" {
			staticName = name
		}
	}
	if staticName == "" {
		return "", nil, fmt.Errorf("failed to locate libslint_cpp static artifact in %s", config.artifactDir)
	}
	return staticName, copied, nil
}

func readPackageName(outputFile string) (string, error) {
	file, err := os.Open(outputFile)
	if err != nil {
		return "", err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if value, ok := strings.CutPrefix(line, "package "); ok {
			return strings.TrimSpace(value), nil
		}
	}
	if err := scanner.Err(); err != nil {
		return "", err
	}
	return "", fmt.Errorf("failed to determine package name from %s", outputFile)
}

func unixSharedFlags(config *buildConfig) string {
	flags := []string{
		"-L${SRCDIR}/.slint-build/dist/" + config.targetID,
		"-lslint_cpp",
	}
	if config.goos == "linux" {
		flags = append(flags, "-ldl", "-lm", "-lpthread")
	}
	return strings.Join(flags, " ")
}

func staticFlags(config *buildConfig, staticName string) string {
	flags := []string{
		"${SRCDIR}/.slint-build/dist/" + config.targetID + "/" + staticName,
	}
	if config.goos == "linux" {
		flags = append(flags, "-ldl", "-lm", "-lpthread", "-lfontconfig")
	}
	return strings.Join(flags, " ")
}

func windowsSharedFlags(config *buildConfig, copied []string) string {
	flags := []string{"${SRCDIR}/.slint-build/dist/" + config.targetID, "slint_cpp.dll"}
	for _, name := range copied {
		if strings.HasSuffix(name, ".dll") {
			continue
		}
		if strings.Contains(name, "slint_cpp") && (strings.HasSuffix(name, ".dll.a") || strings.HasSuffix(name, ".lib")) {
			return strings.Join(flags, " ")
		}
	}
	return strings.Join(flags, " ")
}

func buildTags(config *buildConfig, slintStatic bool) (string, string) {
	modern := []string{config.goos, config.goarch}
	legacy := []string{config.goos, config.goarch}
	if slintStatic {
		modern = append(modern, "slint_static")
		legacy = append(legacy, "slint_static")
	} else {
		modern = append(modern, "!slint_static")
		legacy = append(legacy, "!slint_static")
	}
	return strings.Join(modern, " && "), strings.Join(legacy, ",")
}

func runtimeFileName(config *buildConfig, slintStatic bool) string {
	mode := "shared"
	if slintStatic {
		mode = "static"
	}
	return filepath.Join(
		config.outputDir,
		fmt.Sprintf("runtime_%s_%s_%s.go", config.goos, config.goarch, mode),
	)
}

func runtimeFileContents(packageName string, config *buildConfig, slintStatic bool, staticName string, copied []string) string {
	modern, legacy := buildTags(config, slintStatic)
	flags := unixSharedFlags(config)
	if config.goos == "windows" {
		flags = windowsSharedFlags(config, copied)
	}
	if slintStatic {
		flags = staticFlags(config, staticName)
	}
	return fmt.Sprintf(`// Code generated by Slint. DO NOT EDIT.

//go:build %s
// +build %s

package %s

/*
#cgo LDFLAGS: %s
*/
import "C"
`, modern, legacy, packageName, flags)
}

func writeRuntimeFiles(packageName string, config *buildConfig, staticName string, copied []string) error {
	for _, slintStatic := range []bool{false, true} {
		path := runtimeFileName(config, slintStatic)
		contents := runtimeFileContents(packageName, config, slintStatic, staticName, copied)
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			return err
		}
	}
	return nil
}

func main() {
	var output string
	flag.StringVar(&output, "o", "", "output Go file")
	flag.Parse()

	if output == "" || flag.NArg() != 1 {
		fmt.Fprintln(os.Stderr, "usage: SLINT_GOOS=<goos> SLINT_GOARCH=<goarch> slint-go -o <output.go> <input.slint>")
		os.Exit(2)
	}

	root := repoRoot()
	input, err := filepath.Abs(flag.Arg(0))
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to resolve input path: %v\n", err)
		os.Exit(1)
	}
	output, err = filepath.Abs(output)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to resolve output path: %v\n", err)
		os.Exit(1)
	}

	config, err := newBuildConfig(root, output)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to determine build configuration: %v\n", err)
		os.Exit(1)
	}
	if err := ensureBuildLayout(config); err != nil {
		fmt.Fprintf(os.Stderr, "failed to prepare build directory: %v\n", err)
		os.Exit(1)
	}

	env := buildEnv(config)
	if err := run(root, env, "cargo", cargoArgs(config, "slint-cpp")...); err != nil {
		fmt.Fprintf(os.Stderr, "cargo build failed: %v\n", err)
		os.Exit(1)
	}
	staticName, copied, err := stageArtifacts(config)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to stage runtime artifacts: %v\n", err)
		os.Exit(1)
	}
	if err := run(root, env, "cargo", compilerArgs(config, output, input)...); err != nil {
		fmt.Fprintf(os.Stderr, "slint-compiler failed: %v\n", err)
		os.Exit(1)
	}
	packageName, err := readPackageName(output)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to determine generated package name: %v\n", err)
		os.Exit(1)
	}
	if err := writeRuntimeFiles(packageName, config, staticName, copied); err != nil {
		fmt.Fprintf(os.Stderr, "failed to write runtime files: %v\n", err)
		os.Exit(1)
	}
}
