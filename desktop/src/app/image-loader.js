/* app/image-loader.js — progressive, shared chat image loading.

   One store per conversation identity (`MarkdownMediaContext.key`). A store:
   - dedupes in-flight and resolved loads per source, so two cards showing the
     same image share one download;
   - asks for a thumbnail first, then upgrades to the full image, notifying
     every subscriber as each step lands;
   - keeps a validated thumbnail when the full image fails so the reader still
     sees the preview, with a visible status and explicit retry;
   - refcounts consumers: aborting one card's signal only detaches that card,
     while the last one cancels the queued/network request through the host;
   - bounds its cache by entry count and data-URL bytes, evicting only
     entries no card currently shows;
   - never caches a rejected promise: failures stay visible until an explicit
     retry, which starts one fresh request.
   Requests carry a unique id; the backend caps concurrent transfers. */
(function () {
  const CANCEL_MESSAGE = "image request cancelled";
  const DEFAULT_MAX_ENTRIES = 24;
  const DEFAULT_MAX_BYTES = 32 * 1024 * 1024;
  const SKIPPED = Symbol("image request skipped before admission");
  let sequence = 0;

  const now = () =>
    typeof performance !== "undefined" && typeof performance.now === "function"
      ? performance.now()
      : Date.now();

  function nextRequestId() {
    sequence += 1;
    const uuid = globalThis.crypto?.randomUUID?.();
    return uuid ? `img-${uuid}` : `img-${Date.now().toString(36)}-${sequence.toString(36)}`;
  }

  function messageOf(error) {
    return String(error?.message ?? error ?? "");
  }

  function isCancelled(error) {
    return messageOf(error) === CANCEL_MESSAGE || error?.name === "AbortError";
  }

  function toDataUrl(result) {
    return `data:${result.mimeType};base64,${result.data}`;
  }

  /** Error text without URL query/fragment (some host errors echo the source). */
  function redactDetail(message) {
    return String(message ?? "").replace(/((?:https?:\/\/|data:)[^\s?#]*)[?#][^\s]*/g, "$1");
  }

  /** Debug label without query or fragment; a data URL keeps only its type. */
  function redactSource(source) {
    if (typeof source !== "string" || !source) return "";
    if (source.startsWith("data:")) return `${source.slice(0, 40).split(",")[0]},…`;
    const trimmed = source.split(/[?#]/, 1)[0];
    return trimmed.length > 160 ? `${trimmed.slice(0, 157)}…` : trimmed;
  }

  /** Stable backend conversation cache key: a reopened tab keeps its session
   * file and therefore its offline image cache; a new session falls back to
   * the ephemeral tab id. */
  function conversationKey(profileId, sessionFile, tabId) {
    return JSON.stringify([profileId, sessionFile ?? tabId]);
  }

  /** Presentation state for one subscribed image. A decoded full image can
   * still fail (truncated bytes with a valid magic signature), which is an
   * error with retry, not an endless loading placeholder. */
  function deriveImageStatus(snapshot, decode) {
    const thumbUsable = !!snapshot.thumbnailUrl && !decode.thumbBroken;
    const fullUsable = !!snapshot.fullUrl && !decode.fullBroken;
    let displayKind = null;
    if (fullUsable) displayKind = "full";
    else if (thumbUsable) displayKind = "thumbnail";
    const displayUrl =
      displayKind === "full" ? snapshot.fullUrl : displayKind === "thumbnail" ? snapshot.thumbnailUrl : null;

    let status;
    let error = snapshot.error ?? null;
    if (displayKind === "full") {
      status = "ready";
      error = null;
    } else if (displayKind === "thumbnail") {
      if (snapshot.phase === "error" || snapshot.phase === "ready" || snapshot.fullError) {
        status = "full-error";
        error = snapshot.error ?? snapshot.fullError ?? "The full image could not be displayed.";
      } else if (snapshot.phase === "thumbnail") {
        status = "preview";
      } else {
        status = "loading-full";
      }
    } else if (snapshot.phase === "error" || snapshot.phase === "thumbnail") {
      status = "error";
      error = snapshot.error ?? snapshot.fullError ?? "The image could not be decoded.";
    } else if (snapshot.phase === "ready") {
      status = "error";
      error = decode.fullBroken
        ? "The full image could not be decoded."
        : snapshot.error ?? "The image could not be decoded.";
    } else {
      status = "loading";
    }
    return { displayKind, displayUrl, status, error };
  }

  function createStore(options) {
    const key = options.key ?? "";
    const read = options.read;
    const cancel = options.cancel ?? (() => Promise.resolve(false));
    const maxEntries = options.maxEntries ?? DEFAULT_MAX_ENTRIES;
    const maxBytes = options.maxBytes ?? DEFAULT_MAX_BYTES;
    const entries = new Map();
    const logged = new Set();
    let clock = 0;
    let disposed = false;

    function logOnce(tag, detail) {
      if (logged.has(tag)) return;
      logged.add(tag);
      console.debug(`[chat-image] ${tag}`, detail);
    }

    function snapshotOf(record) {
      return Object.freeze({
        source: record.source,
        phase: record.phase,
        url: record.full?.url ?? record.thumb?.url ?? null,
        thumbnailUrl: record.thumb?.url ?? null,
        fullUrl: record.full?.url ?? null,
        error: record.error,
        fullError: record.fullError,
        thumbAt: record.thumb?.at ?? null,
        fullAt: record.full?.at ?? null,
      });
    }

    function notify(record) {
      const snapshot = snapshotOf(record);
      for (const listener of [...record.listeners]) {
        try {
          listener(snapshot);
        } catch (error) {
          console.error("[chat-image] subscriber failed", error);
        }
      }
    }

    function createRecord(source, thumbnailSource) {
      return {
        source,
        thumbnailSource,
        phase: "loading",
        thumb: null,
        full: null,
        error: null,
        fullError: null,
        listeners: new Set(),
        refs: 0,
        requestIds: new Set(),
        epoch: 0,
        lastUsed: ++clock,
      };
    }

    function recordBytes(record) {
      return (record.thumb?.url.length ?? 0) + (record.full?.url.length ?? 0);
    }

    function totalBytes() {
      let bytes = 0;
      for (const record of entries.values()) bytes += recordBytes(record);
      return bytes;
    }

    function evict() {
      while (entries.size > maxEntries || totalBytes() > maxBytes) {
        let victim = null;
        for (const record of entries.values()) {
          if (record.listeners.size > 0 || record.requestIds.size > 0) continue;
          if (!record.thumb && !record.full) {
            victim = record;
            break;
          }
          if (!victim || record.lastUsed < victim.lastUsed) victim = record;
        }
        if (!victim) return;
        entries.delete(victim.source);
      }
    }

    function cancelRequests(record) {
      for (const requestId of record.requestIds) {
        try {
          const result = cancel(requestId);
          if (result && typeof result.catch === "function") {
            result.catch(error => console.debug("[chat-image] cancel failed", messageOf(error)));
          }
        } catch (error) {
          console.debug("[chat-image] cancel failed", messageOf(error));
        }
      }
      record.requestIds.clear();
    }

    function request(record, epoch, source, priority, slot) {
      const requestId = nextRequestId();
      record.requestIds.add(requestId);
      const startedAt = now();

      Promise.resolve()
        .then(() => {
          // A microtask can run after the last consumer already cancelled and
          // the record was released. Never hand that request to the host.
          if (disposed || record.epoch !== epoch || record.listeners.size === 0) {
            record.requestIds.delete(requestId);
            return SKIPPED;
          }
          return read(source, { requestId, priority });
        })
        .then(result => {
          record.requestIds.delete(requestId);
          if (result === SKIPPED || disposed || record.epoch !== epoch) return;
          if (!result || typeof result.data !== "string" || typeof result.mimeType !== "string") {
            throw new Error("The image service returned a malformed response.");
          }
          const url = toDataUrl(result);
          if (slot === "thumb") {
            record.thumb = {
              url,
              data: result.data,
              mimeType: result.mimeType,
              at: now(),
            };
            record.phase = "full-loading";
            record.fullError = null;
            record.lastUsed = ++clock;
            notify(record);
            logOnce("thumbnail", {
              source: redactSource(source),
              ms: Math.round(now() - startedAt),
              cacheHit: result.cacheHit ?? null,
              timing: result.timing ?? null,
            });
            if (record.listeners.size > 0) requestFull(record, epoch);
            return;
          }
          record.full = {
            url,
            data: result.data,
            mimeType: result.mimeType,
            at: now(),
          };
          record.phase = "ready";
          record.error = null;
          record.fullError = null;
          record.lastUsed = ++clock;
          notify(record);
          evict();
          logOnce("full", {
            source: redactSource(source),
            ms: Math.round(now() - startedAt),
            cacheHit: result.cacheHit ?? null,
            timing: result.timing ?? null,
          });
        })
        .catch(error => {
          record.requestIds.delete(requestId);
          if (disposed || record.epoch !== epoch || isCancelled(error)) return;
          const message = messageOf(error) || "Image unavailable";
          if (slot === "thumb") {
            logOnce("thumbnail-error", { source: redactSource(source), message: redactDetail(message) });
            // A thumbnail is an optimisation; one full request remains.
            if (record.listeners.size > 0) requestFull(record, epoch);
            return;
          }
          logOnce("full-error", { source: redactSource(source), message: redactDetail(message) });
          if (record.thumb) {
            record.phase = "thumbnail";
            record.fullError = message;
          } else {
            record.phase = "error";
            record.error = message;
          }
          notify(record);
          evict();
        });
    }

    function requestFull(record, epoch) {
      request(record, epoch, record.source, "full", "full");
    }

    function begin(record) {
      const epoch = record.epoch;
      if (record.thumbnailSource && record.thumbnailSource !== record.source) {
        // An explicit distinct preview source is a validated image of its
        // own; request it at thumbnail priority so the host shows it first.
        request(record, epoch, record.thumbnailSource, "thumbnail", "thumb");
      } else {
        // Without a distinct preview, a thumbnail request followed by a full
        // request would transfer and cache the same bytes twice. One full
        // request is the only download this source gets.
        request(record, epoch, record.source, "full", "full");
      }
    }

    function resume(record) {
      if (record.phase === "error" || record.full) return;
      if (record.requestIds.size > 0) return;
      if (record.phase === "thumbnail") {
        if (record.fullError) return; // failed fulls retry explicitly only
        record.phase = "full-loading";
        record.paused = false;
        notify(record);
        requestFull(record, record.epoch);
        return;
      }
      if (record.phase === "loading" && !record.thumb && !record.full) begin(record);
    }

    function release(record) {
      record.epoch += 1;
      cancelRequests(record);
      if (record.full) {
        record.phase = "ready";
      } else if (record.thumb) {
        // An in-flight full was interrupted: keep the validated preview and
        // resume it if shown again. A failed full stays failed until retry.
        if (!(record.phase === "thumbnail" && record.fullError)) {
          record.phase = "thumbnail";
          record.fullError = null;
          record.paused = true;
        }
      } else if (record.phase !== "error") {
        entries.delete(record.source);
      }
      evict();
    }

    function subscribeImage(source, listener, opts) {
      if (disposed) throw new Error("The chat image store is disposed.");
      if (!source) throw new Error("An image source is required.");
      let record = entries.get(source);
      if (!record) {
        record = createRecord(source, opts?.thumbnailSource);
        entries.set(source, record);
      }
      record.refs += 1;
      record.listeners.add(listener);
      record.lastUsed = ++clock;

      const signal = opts?.signal;
      let attached = true;
      const off = () => {
        if (!attached) return;
        attached = false;
        if (signal) signal.removeEventListener("abort", off);
        record.listeners.delete(listener);
        record.refs = Math.max(0, record.refs - 1);
        if (record.refs === 0) release(record);
      };

      if (signal) {
        if (signal.aborted) {
          off();
          return () => {};
        }
        signal.addEventListener("abort", off, { once: true });
      }
      listener(snapshotOf(record));
      resume(record);
      return off;
    }

    function peekImage(source) {
      const record = entries.get(source);
      if (!record) {
        return Object.freeze({
          source,
          phase: "loading",
          url: null,
          thumbnailUrl: null,
          fullUrl: null,
          error: null,
          fullError: null,
          thumbAt: null,
          fullAt: null,
        });
      }
      return snapshotOf(record);
    }

    function retryImage(source, opts) {
      if (disposed) throw new Error("The chat image store is disposed.");
      let record = entries.get(source);
      if (!record) {
        record = createRecord(source, opts?.thumbnailSource);
        entries.set(source, record);
      }
      record.epoch += 1;
      cancelRequests(record);
      record.error = null;
      record.fullError = null;
      record.paused = false;
      record.lastUsed = ++clock;

      if (record.thumb) {
        record.full = null;
        record.phase = "full-loading";
        notify(record);
        requestFull(record, record.epoch);
      } else {
        record.thumb = null;
        record.full = null;
        record.phase = "loading";
        notify(record);
        begin(record);
      }
      evict();
    }

    function resolveImage(source, opts = {}) {
      return new Promise((resolve, reject) => {
        let done = false;
        let subscription = null;
        let unsubscribeAfterSubscribe = false;

        const abort = () => finish(() => {
          const error = new Error("Image request aborted.");
          error.name = "AbortError";
          reject(error);
        });

        const finish = fn => {
          if (done) return;
          done = true;
          if (opts.signal) opts.signal.removeEventListener("abort", abort);
          if (subscription) {
            const off = subscription;
            subscription = null;
            off();
          } else {
            unsubscribeAfterSubscribe = true;
          }
          fn();
        };

        if (opts.signal) {
          if (opts.signal.aborted) {
            abort();
            return;
          }
          opts.signal.addEventListener("abort", abort, { once: true });
        }

        const controller = new AbortController();
        subscription = subscribeImage(source, snapshot => {
          if (done) return;
          const record = entries.get(source);
          if (record?.full) {
            finish(() => resolve({ data: record.full.data, mimeType: record.full.mimeType }));
          } else if (snapshot.phase === "error") {
            finish(() => reject(new Error(snapshot.error ?? "Image unavailable")));
          } else if (snapshot.phase === "thumbnail" && snapshot.fullError) {
            finish(() => reject(new Error(snapshot.fullError)));
          }
        }, { signal: controller.signal, thumbnailSource: opts.thumbnail });

        if (unsubscribeAfterSubscribe) {
          const off = subscription;
          subscription = null;
          off?.();
        }
      });
    }

    function logDecode(source, kind, ms) {
      logOnce(`decode:${kind}`, {
        source: redactSource(source),
        kind,
        ms: Number.isFinite(ms) ? Math.round(ms) : null,
      });
    }

    function dispose() {
      if (disposed) return;
      disposed = true;
      for (const record of entries.values()) {
        record.epoch += 1;
        record.listeners.clear();
        record.refs = 0;
        cancelRequests(record);
      }
      entries.clear();
    }

    return Object.freeze({
      key,
      peekImage,
      subscribeImage,
      resolveImage,
      retryImage,
      logDecode,
      dispose,
      get size() {
        return entries.size;
      },
    });
  }

  /** React binding for one image, subscribed until unmount or `source` change. */
  function useChatImage(source, options = {}) {
    const thumbnail = options.thumbnail;
    const media = React.useContext(window.MarkdownMediaContext);
    if (!media) throw new Error("MarkdownMediaContext is required to load chat images.");
    const [snapshot, setSnapshot] = React.useState(() => media.peekImage(source));
    const [decode, setDecode] = React.useState({ thumbBroken: false, fullBroken: false });

    React.useEffect(() => {
      setSnapshot(media.peekImage(source));
      setDecode({ thumbBroken: false, fullBroken: false });
    }, [media, source, thumbnail]);

    React.useEffect(() => {
      if (!source) return undefined;
      const controller = new AbortController();
      const unsubscribe = media.subscribeImage(source, setSnapshot, {
        signal: controller.signal,
        thumbnailSource: thumbnail,
      });
      return () => {
        controller.abort();
        unsubscribe();
      };
    }, [media, source, thumbnail]);

    const { displayKind, displayUrl, status, error } = deriveImageStatus(snapshot, decode);

    const onLoad = React.useCallback(() => {
      const at = displayKind === "full" ? snapshot.fullAt : snapshot.thumbAt;
      const elapsed = at ? now() - at : 0;
      if (displayKind) media.logDecode(source, displayKind, elapsed);
    }, [displayKind, media, source, snapshot.fullAt, snapshot.thumbAt]);

    const onError = React.useCallback(() => {
      if (displayKind === "thumbnail") {
        setDecode(current => (current.thumbBroken ? current : { ...current, thumbBroken: true }));
      } else if (displayKind === "full") {
        setDecode(current => (current.fullBroken ? current : { ...current, fullBroken: true }));
      }
      media.logDecode(source, `${displayKind ?? "image"}-decode-error`, 0);
    }, [displayKind, media, source]);

    const retry = React.useCallback(() => {
      setDecode({ thumbBroken: false, fullBroken: false });
      media.retryImage(source, { thumbnailSource: thumbnail });
    }, [media, source, thumbnail]);

    return {
      source: snapshot.source,
      phase: snapshot.phase,
      status,
      error,
      displayUrl,
      displayKind,
      retry,
      onLoad,
      onError,
    };
  }

  window.OMP_CHAT_IMAGE = Object.freeze({ createStore, useChatImage, conversationKey, deriveImageStatus, redactSource });
})();
