//go:build !slint_static
// +build !slint_static

package slint

/*
#cgo LDFLAGS: -L${SRCDIR}/../../../target/debug -lslint_cpp -ldl -lm -lpthread
*/
import "C"
