// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Blocks produced by the reference LZ4 implementation (python-lz4 4.x over
// upstream liblz4, high compression), so that src/lz4.js is checked against
// the format itself and not only against its own output. The inputs are
// formulas rather than data: the point of the fixture is the blocks.
//
// Regenerate with tools/lz4fixtures.py if the set ever needs to grow.

const enc = new TextEncoder();
const count = (n) => Uint8Array.from({ length: n }, (_, i) => i & 0xff);
const zeros = (n) => new Uint8Array(n);
const run = (n, b) => new Uint8Array(n).fill(b);
const period = (n, p) => Uint8Array.from({ length: n }, (_, i) => 97 + (i % p));
const text = (n) => enc.encode('the quick brown fox jumps over the lazy dog. '.repeat(n));
// A match 65535 bytes back is the furthest the two-byte offset field reaches.
const edge = () =>
  Uint8Array.from({ length: 65567 }, (_, i) =>
    i < 32 || i >= 65535 ? 97 + (i % 7) : (i * 37 + 11) & 0xff,
  );

export const REFERENCE_BLOCKS = [
  {
    name: 'empty',
    plain: () => zeros(0),
    block: 'AA==',
  },
  {
    name: 'one byte',
    plain: () => run(1, 65),
    block: 'EEE=',
  },
  {
    name: 'shorter than the minimum match',
    plain: () => count(11),
    block: 'sAABAgMEBQYHCAkK',
  },
  {
    name: 'exactly one group',
    plain: () => count(13),
    block: '0AABAgMEBQYHCAkKCww=',
  },
  {
    name: 'a literal run of fifteen',
    plain: () => count(15),
    block: '8AAAAQIDBAUGBwgJCgsMDQ4=',
  },
  {
    name: 'a literal run needing a continuation byte',
    plain: () => count(300),
    block:
      '//EAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSYnKCkqKywtLi8w' +
      'MTIzNDU2Nzg5Ojs8PT4/QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl9gYWJj' +
      'ZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXp7fH1+f4CBgoOEhYaHiImKi4yNjo+QkZKTlJWW' +
      'l5iZmpucnZ6foKGio6SlpqeoqaqrrK2ur7CxsrO0tba3uLm6u7y9vr/AwcLDxMXGx8jJ' +
      'ysvMzc7P0NHS09TV1tfY2drb3N3e3+Dh4uPk5ebn6Onq6+zt7u/w8fLz9PX29/j5+vv8' +
      '/f7/AAEUUCcoKSor',
  },
  {
    name: 'five thousand zeros',
    plain: () => zeros(5000),
    block: 'HwABAP////////////////////////+CUAAAAAAA',
  },
  {
    name: 'a three byte period',
    plain: () => period(2100, 3),
    block: 'P2FiYwMA//////////8hUGJjYWJj',
  },
  {
    name: 'a match at the far edge of the offset field',
    plain: () => edge(),
    block:
      'f2FiY2RlZmcHAAb/8avQ9Ro/ZImu0/gdQmeMsdb7IEVqj7TZ/iNIbZK33AEmS3CVut8E' +
      'KU5zmL3iByxRdpvA5QovVHmew+gNMld8ocbrEDVaf6TJ7hM4XYKnzPEWO2CFqs/0GT5j' +
      'iK3S9xxBZouw1fofRGmOs9j9IkdskbbbACVKb5S53gMoTXKXvOEGK1B1mr/kCS5TeJ3C' +
      '5wwxVnugxeoPNFl+o8jtEjdcgabL8BU6X4SpzvMYPWKHrNH2G0Bliq/U+R5DaI2y1/wh' +
      'RmuQtdr/JEluk7jdAidMcZa74AUqT3SZvuMILVJ3nMHmCzBVep/E6Q4zWH2ix+wRNluA' +
      'pcrvFDleg6jN8hc8YYYAAf//////////////////////////////////////////////' +
      '////////////////////////////////////////////////////////////////////' +
      '////////////////////////////////////////////////////////////////////' +
      '////////////////////////////////////////////////////////////////////' +
      '////////////////////////////////////////////////////////////////////' +
      '/////////////////////8sP/v8IUGFiY2Rl',
  },
  {
    name: 'text',
    plain: () => text(40),
    block:
      '8BB0aGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgHwCvbGF6eSBkb2cuIC0A////' +
      '////yVBkb2cuIA==',
  },
];
