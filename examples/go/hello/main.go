package main

//go:generate go run github.com/slint-ui/slint/api/go/cmd/slint-go -o ui/app.slint.go app.slint

import (
	"fmt"
	"strings"

	"github.com/slint-ui/slint/examples/go/hello/ui"
)

func main() {
	window, err := ui.NewAppWindow()
	if err != nil {
		panic(err)
	}
	if err := window.SetName("Gophers"); err != nil {
		panic(err)
	}
	if err := window.Logic().OnMakeGreeting(func(name string) string {
		return fmt.Sprintf("Hello, %s!", strings.ToUpper(name))
	}); err != nil {
		panic(err)
	}
	if err := window.Run(); err != nil {
		panic(err)
	}
}
