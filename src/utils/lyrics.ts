export type ParsedLyricLine = {
  key: string;
  time: number | null;
  text: string;
};

export function parse_lyric_lines(lyrics: string) {
  const parsed: ParsedLyricLine[] = [];

  lyrics.split(/\r?\n/).forEach((source_line, source_index) => {
    const line = source_line.trim();
    if (!line) return;
    if (/^\[[a-z]+:/iu.test(line)) return;

    const time_matches = [...line.matchAll(/\[(\d{1,2}):(\d{2})(?:[.:](\d{1,3}))?\]/gu)];
    const text = line.replace(/^(\[[^\]]+\])+\s*/u, "").trim();
    if (!text) return;

    if (!time_matches.length) {
      parsed.push({
        key: `plain-${source_index}`,
        time: null,
        text,
      });
      return;
    }

    for (const match of time_matches) {
      const minute = Number(match[1]);
      const second = Number(match[2]);
      const fraction = match[3] ?? "0";
      const millisecond = Number(fraction.padEnd(3, "0").slice(0, 3));
      parsed.push({
        key: `${minute}-${second}-${millisecond}-${source_index}`,
        time: minute * 60 + second + millisecond / 1000,
        text,
      });
    }
  });

  return parsed.sort((left, right) => {
    if (left.time === null && right.time === null) return 0;
    if (left.time === null) return 1;
    if (right.time === null) return -1;
    return left.time - right.time;
  });
}

export function has_timed_lyric_lines(lines: ParsedLyricLine[]) {
  return lines.some((line) => line.time !== null);
}

export function lyric_index_for_elapsed(lines: ParsedLyricLine[], seconds: number) {
  if (!has_timed_lyric_lines(lines)) return -1;

  let index = -1;
  for (let current = 0; current < lines.length; current += 1) {
    const time = lines[current].time;
    if (time === null) continue;
    if (time <= seconds + 0.08) {
      index = current;
    } else {
      break;
    }
  }
  if (index >= 0) return index;
  return lines.findIndex((line) => line.time !== null);
}
