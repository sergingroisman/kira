let segmenter: Intl.Segmenter | null = null;

export function getGraphemeSegmenter(): Intl.Segmenter {
  if (!segmenter) {
    segmenter = new Intl.Segmenter('en', { granularity: 'grapheme' });
  }
  return segmenter;
}
