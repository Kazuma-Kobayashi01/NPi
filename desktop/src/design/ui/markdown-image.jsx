/* Inline images resolve through the conversation-bound host API. */
(function () {
  function MarkdownInlineImage({ src, alt, title }) {
    const media = React.useContext(window.MarkdownMediaContext);
    const onLink = React.useContext(window.MarkdownLinkContext);
    const [result, setResult] = React.useState({ kind: "loading" });
    const [openIndex, setOpenIndex] = React.useState(null);
    const imgRef = React.useRef(null);
    const thumbFor = React.useCallback(() => imgRef.current, []);
    const close = React.useCallback(() => setOpenIndex(null), []);

    React.useEffect(() => {
      let current = true;
      media.resolveImage(src).then(({ data, mimeType }) => {
        if (current) setResult({ kind: "loaded", url: `data:${mimeType};base64,${data}` });
      }).catch(error => {
        if (current) setResult({ kind: "error", message: String(error.message ?? error) });
      });
      return () => { current = false; };
    }, [media, src]);

    const sourceLink = /^https:\/\//i.test(src) && (
      <a href={src} target="_blank" rel="noopener noreferrer" className="md-image-attribution"
        title={src} onClick={e => { e.stopPropagation(); onLink?.(e); }}>
        Source: {src}
      </a>
    );

    if (result.kind === "loading") {
      return <span className="md-image-loading"><span className="pulse-dot" /> Loading {alt || "image"}…</span>;
    }
    if (result.kind === "error") {
      return <span className="md-image-error">
        <span className="chip danger">Image unavailable</span>
        <span>{result.message}</span>
        {sourceLink || <span className="md-image-error-src">{src}</span>}
      </span>;
    }

    return <span className="md-image-card">
      <button type="button" className="md-image-btn" aria-label={`Enlarge image: ${alt || title || src}`}
        onClick={e => { e.stopPropagation(); setOpenIndex(0); }}>
        <img ref={imgRef} src={result.url} alt={alt || ""} title={title || alt || ""}
          className="md-inline-img" onError={() => setResult({ kind: "error", message: "The image could not be decoded." })} />
      </button>
      {(title || alt) && <span className="md-image-caption">{title || alt}</span>}
      {sourceLink}
      {openIndex !== null && <window.ImageLightbox images={[result.url]} index={openIndex}
        onIndex={setOpenIndex} onClose={close} thumbFor={thumbFor} />}
    </span>;
  }
  window.MarkdownInlineImage = MarkdownInlineImage;
})();
