export function extractLines(fileContent: string, start: number, end: number): string {
  return fileContent.split('\n').slice(start - 1, end).join('\n');
}