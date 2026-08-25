#!/usr/bin/env node
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from 'node:fs';
import { encode, decode, decodeDetailed, Base91JdpError } from '../src/index.js';

const USAGE = `usage: base91jdp [options] [file]

Encode bytes to base91-jdp, or decode them back. Reads standard input when no
file is given, and writes to standard output with no trailing newline.

By default the encoder tries LZ4 and passthrough and keeps whichever is
shorter, so there is nothing to choose unless you want error correction.

  -d, --decode        decode instead of encode
  -w, --wrap <n>      wrap encoded output every n characters (0 = off, default)
  -z, --compress <m>  auto (default), never, always
  -p, --protect <m>   auto (default), check, yes, no
                        yes    Reed-Solomon: a flipped bit is repaired
                        check  damage is reported, not repaired, and free
                        no     neither
      --partial       when decoding, keep the segments that survived
  -v, --verbose       when decoding, report the mode and any damage on stderr
  -h, --help          show this help
  -V, --version       show the version

Examples:
  base91jdp photo.jpg > photo.b91
  base91jdp -d photo.b91 > photo.jpg
  base91jdp --protect yes backup.tar > backup.b91
  base91jdp -d --partial --verbose damaged.b91 > salvaged.tar
`;

const PROTECT = { auto: 'auto', check: 'check', yes: true, no: false };

function main(argv) {
  let decodeMode = false;
  let wrap = 0;
  let file = null;
  let compress = 'auto';
  let protect = 'auto';
  let partial = false;
  let verbose = false;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '-d' || arg === '--decode') decodeMode = true;
    else if (arg === '--partial') partial = true;
    else if (arg === '-v' || arg === '--verbose') verbose = true;
    else if (arg === '-z' || arg === '--compress') {
      compress = argv[++i];
      if (!['auto', 'never', 'always'].includes(compress)) fatal(`bad --compress value ${compress}`);
    } else if (arg === '-p' || arg === '--protect') {
      const name = argv[++i];
      if (!(name in PROTECT)) fatal(`bad --protect value ${name}`);
      protect = PROTECT[name];
    } else if (arg === '-w' || arg === '--wrap') {
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
    const text = input.toString('latin1');
    if (!verbose) {
      process.stdout.write(decode(text, { partial }));
      return 0;
    }
    const seen = decodeDetailed(text);
    const what = seen.framed ? seen.mode : 'headerless';
    process.stderr.write(
      `base91jdp: ${what}, ${seen.segments} segment(s)` +
        (seen.repaired ? `, ${seen.repaired} symbol(s) repaired` : '') +
        (seen.damaged.length ? `, ${seen.damaged.length} segment(s) lost` : '') +
        '\n',
    );
    for (const { segment, trouble } of seen.damaged) {
      process.stderr.write(`base91jdp:   segment ${segment}: ${trouble[0].reason}\n`);
    }
    if (seen.damaged.length && !partial) {
      fatal(`${seen.damaged.length} segment(s) could not be recovered; --partial keeps the rest`);
    }
    process.stdout.write(seen.bytes);
    return 0;
  }

  let out = encode(new Uint8Array(input), { compress, protect });
  if (wrap > 0) out = out.replace(new RegExp(`(.{${wrap}})`, 'g'), '$1\n');
  process.stdout.write(out);
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
