export const color = {
  yellowPrimary: '#FFDE00',
  bluePrimary: '#6FC2FF',
  tealAccent: '#53DBC9',
  redAccent: '#FF7169',
  bgBeige: '#F4EFEA',
  bgOffwhite: '#F8F8F7',
  bgWhite: '#FFFFFF',
  bgLightGray: '#F1F1F1',
  textDark: '#383838',
  textGray: '#818181',
  textLightGray: '#A1A1A1',
  darkSurface: '#383838',
} as const;

export const font = {
  mono: '"JetBrains Mono", "PingFang SC", "Microsoft YaHei", monospace',
} as const;

export const size = {
  display: 56, h2: 24, h3: 14, eyebrow: 10,
  bodyLg: 16, body: 14, bodySm: 13, bodyXs: 12,
  caption: 11, mini: 10,
} as const;

export const space = {
  px4: 4, px6: 6, px8: 8, px10: 10, px12: 12,
  px16: 16, px20: 20, px24: 24, px32: 32, px48: 48,
} as const;

export const border = {
  thin: `1px solid ${color.textDark}`,
  std: `2px solid ${color.textDark}`,
  thick: `3px solid ${color.textDark}`,
  dashed: `1px dashed ${color.bgLightGray}`,
  dashedSection: `2px dashed ${color.bgLightGray}`,
  radius: '0px',
} as const;

export const transition = {
  btn: 'transform 0.12s ease-in-out',
  card: '0.4s ease-out',
  nav: 'background-color 0.2s ease-in-out',
} as const;
