package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

func repoRoot() string {
	_, file, _, _ := runtime.Caller(0)
	return filepath.Clean(filepath.Join(filepath.Dir(file), "../../.."))
}

func run(dir string, name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	return cmd.Run()
}

func main() {
	var output string
	flag.StringVar(&output, "o", "", "output Go file")
	flag.Parse()

	if output == "" || flag.NArg() != 1 {
		fmt.Fprintln(os.Stderr, "usage: slint-go -o <output.go> <input.slint>")
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

	if err := run(root, "cargo", "build", "-p", "slint-cpp", "--features", "interpreter"); err != nil {
		fmt.Fprintf(os.Stderr, "cargo build failed: %v\n", err)
		os.Exit(1)
	}
	if err := run(root, "cargo", "run", "-p", "slint-compiler", "--", "-f", "go", "-o", output, input); err != nil {
		fmt.Fprintf(os.Stderr, "slint-compiler failed: %v\n", err)
		os.Exit(1)
	}
}
