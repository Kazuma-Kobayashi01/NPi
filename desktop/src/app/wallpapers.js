// The chat wallpaper's artworks and how one is chosen. Exposes
// `window.OMP_WALLPAPERS`; an IIFE (AGENTS.md "IIFE rule"), dependency-free.
//
// The files are faint single-colour masks (assets/wallpapers/CREDITS.md);
// the app tints them with the theme's text colour at a low opacity.
(function () {
  const DIR = "assets/wallpapers/";
  const LIST = Object.freeze([
    { id: "vitruvian-man", title: "Vitruvian Man", artist: "Leonardo da Vinci", year: "c. 1490" },
    { id: "school-of-athens", title: "The School of Athens", artist: "Raphael", year: "1509–1511" },
    { id: "birth-of-venus", title: "The Birth of Venus", artist: "Sandro Botticelli", year: "c. 1485" },
    { id: "creation-of-adam", title: "The Creation of Adam", artist: "Michelangelo", year: "c. 1512" },
    { id: "melencolia", title: "Melencolia I", artist: "Albrecht Dürer", year: "1514" },
    { id: "last-supper", title: "The Last Supper", artist: "Leonardo da Vinci", year: "1495–1498" },
  ].map(a => Object.freeze({ ...a, file: `${DIR}${a.id}.png` })));

  /** A small stable hash of `key`, so "auto" gives each conversation its own artwork. */
  function hash(key) {
    let h = 2166136261;
    for (const ch of String(key ?? "")) h = Math.imul(h ^ ch.codePointAt(0), 16777619);
    return h >>> 0;
  }

  /** The artwork for the setting `art` ("auto" or an id); "auto" is chosen
   *  per `key` (a conversation's id). An unknown id falls back to "auto". */
  function pick(art, key) {
    return LIST.find(a => a.id === art) ?? LIST[hash(key) % LIST.length];
  }

  window.OMP_WALLPAPERS = { LIST, pick, hash };
})();
