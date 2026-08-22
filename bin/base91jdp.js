#!/usr/bin/env node
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from 'node:fs';
import { encode, decode, Base91JdpError } from '../src/index.js';

const USAGE = `usage: base91jdp [options] [file]

Encode bytes to base91-jdp, or decode them back. Reads standard input when no
file is given, and writes to standard output with no trailing newline.

  -d, --decode        decode instead of encode
  -w, --wrap <n>      wrap encoded output every n characters (0 = off, default)
  -h, --help          show this help
  -V, --version       show the version

Examples:
  base91jdp photo.jpg > photo.b91
  base91jdp -d photo.b91 > photo.jpg
  gzip -9 < dump.bin | base91jdp --wrap 100
`;

function main(argv) {
  let decodeMode = false;
  let wrap = 0;
  let file = null;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '-d' || arg === '--decode') decodeMode = true;
    else if (arg === '-w' || arg === '--wrap') {
      wrap = Number(argv[++i]);
      if (!Number.isInteger(wrap) || wrap < 0) fatal(`bad --wrap value`);
    } else if (arg === '-h' || arg === '--help') {
      process.stdout.write(USAGE);
      return 0;
    } else if (arg === '-V' || arg === '--version') {
      const pkg = JSON.parse(
        readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
      );
      process.stdout.write(`${pkg.version}\n`);
      return 0;
    } else if (arg.startsWith('-') && arg !== '-') {
      fatal(`unknown option ${arg}`);
    } else if (file === null) {
      file = arg;
    } else {
      fatal('at most one file');
    }
  }

  const input = readFileSync(file === null || file === '-' ? 0 : file);

  if (decodeMode) {
    process.stdout.write(decode(input.toString('latin1')));
  } else {
    let out = encode(new Uint8Array(input));
    if (wrap > 0) out = out.replace(new RegExp(`(.{${wrap}})`, 'g'), '$1\n');
    process.stdout.write(out);
  }
  return 0;
}

function fatal(message) {
  process.stderr.write(`base91jdp: ${message}\n`);
  process.exit(2);
}

try {
  process.exitCode = main(process.argv.slice(2));
} catch (err) {
  if (err instanceof Base91JdpError) fatal(`${err.code}: ${err.message}`);
  fatal(err.message);
}
