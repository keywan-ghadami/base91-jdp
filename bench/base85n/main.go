// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Encoded sizes for Base85N v0.5.1, the codec base91-jdp is compared against.
//
// The numbers in bench/results/RESULTS.md are measured with the upstream
// implementation rather than quoted from its documentation, which is what
// this program is for. Every file is also decoded again, so a size that ends
// up in the table came from a round trip that worked.
//
//	go run ./bench/base85n ../corpus/*  >  sizes.tsv
package main

import (
	"bytes"
	"fmt"
	"os"

	base85n "github.com/keywan-ghadami/base85n/go"
)

func main() {
	for _, path := range os.Args[1:] {
		data, err := os.ReadFile(path)
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		enc := base85n.Encode(data)
		dec, err := base85n.Decode(enc)
		if err != nil || !bytes.Equal(dec, data) {
			fmt.Fprintf(os.Stderr, "round trip failed for %s: %v\n", path, err)
			os.Exit(1)
		}
		fmt.Printf("%s\t%d\t%d\n", path, len(data), len(enc))
	}
}
