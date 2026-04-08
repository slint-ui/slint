//go:build slint_static
// +build slint_static

package slint

/*
#cgo LDFLAGS: -L${SRCDIR}/../../../target/debug ${SRCDIR}/../../../target/debug/libslint_cpp.a -ldl -lm -lpthread -lfontconfig
*/
import "C"
