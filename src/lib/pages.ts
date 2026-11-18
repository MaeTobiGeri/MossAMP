import { LibraryBig, Search, Settings2, SlidersVertical } from "@lucide/svelte";

export type PageId = "library" | "search" | "eq" | "settings";

export const pages = [
  { id: "library" as PageId, label: "Library", icon: LibraryBig, solid: true },
  { id: "search" as PageId, label: "Search", icon: Search, solid: false },
  { id: "eq" as PageId, label: "EQ", icon: SlidersVertical, solid: true },
  { id: "settings" as PageId, label: "Settings", icon: Settings2, solid: true },
];
