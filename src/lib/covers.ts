import { api } from "./api";

const cache = new Map<string, Promise<string | null>>();

export function coverFor(path: string): Promise<string | null> {
  let cover = cache.get(path);
  if (!cover) {
    cover = api.getCover(path).catch(() => null);
    cache.set(path, cover);
  }
  return cover;
}
