// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What one flipped bit costs Base85N.
//
// The mirror of M1 in bench/rsstudy.js, run against the upstream Go
// implementation so the comparison is with the real codec rather than with a
// reimplementation of it. Base85N's block mode is already byte-synchronous --
// five characters carry exactly four bytes -- so the interesting question is
// not the block coder but the signals: a damaged Dynamic Passthrough signal
// moves a segment length, and a damaged Fill signal invents up to 2048 bytes
// that were never there.
//
//	go run ./bench/base85n/damage <file> [trials]
package main

import (
	"fmt"
	"math/rand"
	"os"
	"sort"
	"strconv"

	base85n "github.com/keywan-ghadami/base85n/go"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: damage <file> [trials]")
		os.Exit(2)
	}
	trials := 3000
	if len(os.Args) > 2 {
		n, err := strconv.Atoi(os.Args[2])
		if err == nil {
			trials = n
		}
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}

	encoded := []byte(base85n.Encode(data))
	rng := rand.New(rand.NewSource(20260823))

	// Accepted and rejected are kept apart on purpose. A decoder that refuses
	// a damaged stream has still lost the payload, but it said so; one that
	// returns different bytes without complaint is a different kind of
	// problem, and lumping the two together would hide it.
	accepted := make([]int, 0, trials)
	var rejected, expanded, silentOverMB int

	for i := 0; i < trials; i++ {
		bad := make([]byte, len(encoded))
		copy(bad, encoded)
		pos := rng.Intn(len(bad))
		// bit 7 would leave printable ASCII, which every reader rejects on
		// sight; the interesting flips are the ones that stay plausible
		bad[pos] ^= 1 << uint(rng.Intn(7))

		out, err := base85n.Decode(string(bad))
		if err != nil {
			rejected++
			continue
		}
		if len(out) > len(data) {
			expanded++
		}
		w := diff(data, out)
		if w > 1<<20 {
			silentOverMB++
		}
		accepted = append(accepted, w)
	}

	sort.Ints(accepted)
	at := func(q float64) int {
		if len(accepted) == 0 {
			return 0
		}
		return accepted[min(len(accepted)-1, int(float64(len(accepted))*q))]
	}
	fmt.Printf("input\t%d\nencoded\t%d\ntrials\t%d\n", len(data), len(encoded), trials)
	fmt.Printf("rejected\t%d\naccepted\t%d\n", rejected, len(accepted))
	fmt.Printf("accepted_median\t%d\naccepted_p95\t%d\naccepted_max\t%d\n",
		at(0.5), at(0.95), accepted[len(accepted)-1])
	fmt.Printf("silently_wrong\t%d\nsilently_over_1MB\t%d\nexpanded\t%d\n",
		countNonZero(accepted), silentOverMB, expanded)
}

func countNonZero(xs []int) int {
	n := 0
	for _, x := range xs {
		if x != 0 {
			n++
		}
	}
	return n
}

func diff(a, b []byte) int {
	n := len(a)
	if len(b) < n {
		n = len(b)
	}
	wrong := 0
	for i := 0; i < n; i++ {
		if a[i] != b[i] {
			wrong++
		}
	}
	return wrong + abs(len(a)-len(b))
}

func abs(x int) int {
	if x < 0 {
		return -x
	}
	return x
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
