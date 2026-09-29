import type { Theme } from '../types';

export interface CollectionTheme {
  id: string;
  name: string;
  nameZh: string;
  description: string;
  descriptionZh: string;
  colors: string[];
  subjectColors: string[];
}

/** The user-approved directions, with matching subject color choices. The
 *  first entry is the default for new installs. */
export const collection: CollectionTheme[] = [
  {
    id: 'classic-tomato', name: 'Classic Tomato', nameZh: '经典番茄',
    description: 'Warm paper label · tomato red · leaf · navy', descriptionZh: '暖纸标签 · 番茄红 · 叶绿 · 藏蓝',
    colors: ['#C83224', '#F5EBD8', '#244B36'],
    // The eight tomato varieties, in their fixed order (varieties.ts; tests keep the two in sync).
    subjectColors: ['#D33526', '#E9A60C', '#4E4AA8', '#F07A22', '#2B7F3E', '#F27C8E', '#8CC63F', '#9C2F55'],
  },
  {
    id: 'cherry-soda', name: 'Cherry Soda', nameZh: '樱桃汽水',
    description: 'Cherry red · candy pink · lemon', descriptionZh: '樱桃红 · 糖果粉 · 柠檬黄',
    colors: ['#B80E46', '#F9A0B7', '#F7E677'],
    subjectColors: ['#B80E46', '#901037', '#C63961', '#E7638E', '#F7E677', '#D5B82D', '#45684A', '#719A61', '#6B548D', '#AA76A5'],
  },
  {
    id: 'citrus-club', name: 'Citrus Club', nameZh: '蓝柠午后',
    description: 'Cobalt blue · citrus yellow', descriptionZh: '钴蓝 · 柠檬黄 · 清爽青绿',
    colors: ['#2454A1', '#F8E6A0', '#FFBE19'],
    subjectColors: ['#2454A1', '#164486', '#5484C3', '#7FB4E5', '#D39208', '#F3BB22', '#08616B', '#3B9893', '#A95159', '#D27C81'],
  },
  {
    id: 'berry-planet', name: 'Berry Planet', nameZh: '莓果星球',
    description: 'Violet · berry pink · starlight', descriptionZh: '星球紫 · 莓果粉 · 星光黄',
    colors: ['#6650B4', '#FF9DD0', '#FFEE9D'],
    subjectColors: ['#6650B4', '#52328F', '#9277D2', '#B5A0E8', '#9E205C', '#D3468A', '#F687BA', '#C09823', '#E4BD58', '#4F858D'],
  },
];

export function collectionFor(theme: Theme | string | null): CollectionTheme | undefined {
  const name = typeof theme === 'string' ? theme : theme?.name;
  return collection.find(t => t.name === name);
}

const legacyPalette = ['#e06c75', '#e59f4a', '#e5c07b', '#98c379', '#56b6c2', '#61afef', '#7e8ce0', '#c678dd', '#d47ba8', '#8b9bb4'];

export function subjectPaletteFor(theme: Theme | null): string[] {
  return collectionFor(theme)?.subjectColors ?? legacyPalette;
}
