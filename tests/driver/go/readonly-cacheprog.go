// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package main

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

type cmd string

const (
	cmdGet   cmd = "get"
	cmdClose cmd = "close"
)

type request struct {
	ID       int64 `json:"ID"`
	Command  cmd   `json:"Command"`
	ActionID []byte
	BodySize int64 `json:",omitempty"`
}

type response struct {
	ID            int64      `json:"ID"`
	Err           string     `json:",omitempty"`
	KnownCommands []cmd      `json:",omitempty"`
	Miss          bool       `json:",omitempty"`
	OutputID      []byte     `json:",omitempty"`
	Size          int64      `json:",omitempty"`
	Time          *time.Time `json:",omitempty"`
	DiskPath      string     `json:",omitempty"`
}

const (
	hashSize  = 32
	hexSize   = hashSize * 2
	entrySize = 2 + 1 + hexSize + 1 + hexSize + 1 + 20 + 1 + 20 + 1
)

func main() {
	enc := json.NewEncoder(os.Stdout)
	if err := enc.Encode(response{ID: 0, KnownCommands: []cmd{cmdGet, cmdClose}}); err != nil {
		fmt.Fprintf(os.Stderr, "failed to write capabilities: %v\n", err)
		os.Exit(1)
	}

	cacheDir := os.Getenv("GOCACHE")
	if cacheDir == "" {
		fmt.Fprintln(os.Stderr, "GOCACHE is not set")
		os.Exit(1)
	}

	dec := json.NewDecoder(os.Stdin)
	for {
		var req request
		if err := dec.Decode(&req); err != nil {
			if errors.Is(err, os.ErrClosed) {
				return
			}
			if errors.Is(err, io.EOF) {
				return
			}
			fmt.Fprintf(os.Stderr, "failed to decode request: %v\n", err)
			os.Exit(1)
		}

		if req.BodySize > 0 {
			var discard string
			if err := dec.Decode(&discard); err != nil {
				fmt.Fprintf(os.Stderr, "failed to discard request body: %v\n", err)
				os.Exit(1)
			}
		}

		var res response
		res.ID = req.ID

		switch req.Command {
		case cmdGet:
			entry, err := loadEntry(cacheDir, req.ActionID)
			if err != nil {
				res.Miss = true
				break
			}
			res.OutputID = entry.OutputID
			res.Size = entry.Size
			res.Time = &entry.Time
			res.DiskPath = entry.DiskPath
		case cmdClose:
			if err := enc.Encode(res); err != nil {
				fmt.Fprintf(os.Stderr, "failed to write close response: %v\n", err)
				os.Exit(1)
			}
			return
		default:
			res.Err = fmt.Sprintf("unsupported command %q", req.Command)
		}

		if err := enc.Encode(res); err != nil {
			fmt.Fprintf(os.Stderr, "failed to write response: %v\n", err)
			os.Exit(1)
		}
	}
}

type cacheEntry struct {
	OutputID []byte
	Size     int64
	Time     time.Time
	DiskPath string
}

func loadEntry(cacheDir string, actionID []byte) (*cacheEntry, error) {
	if len(actionID) != hashSize {
		return nil, errors.New("unexpected action id size")
	}
	actionFile := cacheFileName(cacheDir, actionID, "a")
	data, err := os.ReadFile(actionFile)
	if err != nil {
		return nil, err
	}
	if len(data) != entrySize {
		return nil, errors.New("invalid cache entry size")
	}
	if string(data[:3]) != "v1 " || data[entrySize-1] != '\n' {
		return nil, errors.New("invalid cache entry header")
	}
	if data[3+hexSize] != ' ' || data[3+hexSize+1+hexSize] != ' ' || data[3+hexSize+1+hexSize+1+20] != ' ' {
		return nil, errors.New("invalid cache entry separators")
	}

	entryActionHex := data[3 : 3+hexSize]
	entryOutputHex := data[3+hexSize+1 : 3+hexSize+1+hexSize]
	entrySizeField := data[3+hexSize+1+hexSize+1 : 3+hexSize+1+hexSize+1+20]
	entryTimeField := data[3+hexSize+1+hexSize+1+20+1 : entrySize-1]

	entryActionID, err := hex.DecodeString(string(entryActionHex))
	if err != nil || !bytes.Equal(entryActionID, actionID) {
		return nil, errors.New("mismatched action id")
	}
	outputID, err := hex.DecodeString(string(entryOutputHex))
	if err != nil || len(outputID) != hashSize {
		return nil, errors.New("invalid output id")
	}
	size, err := strconv.ParseInt(strings.TrimSpace(string(entrySizeField)), 10, 64)
	if err != nil || size < 0 {
		return nil, errors.New("invalid size")
	}
	unixNano, err := strconv.ParseInt(strings.TrimSpace(string(entryTimeField)), 10, 64)
	if err != nil || unixNano < 0 {
		return nil, errors.New("invalid time")
	}

	diskPath, err := resolveDiskPath(cacheDir, outputID)
	if err != nil {
		return nil, err
	}

	markUsed(actionFile)
	markUsed(cacheFileName(cacheDir, outputID, "d"))

	return &cacheEntry{
		OutputID: outputID,
		Size:     size,
		Time:     time.Unix(0, unixNano),
		DiskPath: diskPath,
	}, nil
}

func resolveDiskPath(cacheDir string, outputID []byte) (string, error) {
	path := cacheFileName(cacheDir, outputID, "d")
	info, err := os.Stat(path)
	if err != nil {
		return "", err
	}
	if !info.IsDir() {
		return path, nil
	}
	entries, err := os.ReadDir(path)
	if err != nil {
		return "", err
	}
	if len(entries) != 1 {
		return "", errors.New("invalid executable cache entry")
	}
	return filepath.Join(path, entries[0].Name()), nil
}

func cacheFileName(cacheDir string, id []byte, suffix string) string {
	hexID := hex.EncodeToString(id)
	return filepath.Join(cacheDir, hexID[:2], hexID+"-"+suffix)
}

func markUsed(path string) {
	info, err := os.Stat(path)
	if err != nil {
		return
	}
	now := time.Now()
	if now.Sub(info.ModTime()) < time.Hour {
		return
	}
	_ = os.Chtimes(path, now, now)
}
