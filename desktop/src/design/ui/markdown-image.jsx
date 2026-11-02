/* Inline images resolve through the conversation-bound shared image store:
   a preview first, then the full image; a failed full load keeps the
   preview visible with an explicit retry. */
(function () {
  function MarkdownInlineImage({ src, alt, title }) {
    const onLink = React.useContext(window.MarkdownLinkContext);
    const image = window.OMP_CHAT_IMAGE.useChatImage(src);
    const [openIndex, setOpenIndex] = React.useState(null);
    const imgRef = React.useRef(null);
    const thumbFor = React.useCallback(() => imgRef.current, []);
    const close = React.useCallback(() => setOpenIndex(null), []);
    const open = React.useCallback(e => {
      e.stopPropagation();
      setOpenIndex(0);
    }, []);

    const sourceLink = /^https:\/\//i.test(src) && (
      <a href={src} target="_blank" rel="noopener noreferrer" className="md-image-attribution"
        title={src} onClick={e => { e.stopPropagation(); onLink?.(e); }}>
        Source: {src}
      </a>
    );

    if (image.status === "error") {
      return <span className="md-image-error">
        <span className="chip danger">Image unavailable</span>
        <span>{image.error}</span>
        <button type="button" className="md-image-retry" onClick={e => { e.stopPropagation(); image.retry(); }}>Retry image</button>
        {sourceLink || <span className="md-image-error-src">{src}</span>}
      </span>;
    }

    if (!image.displayUrl) {
      return <span className="md-image-loading"><span className="pulse-dot" /> Loading {alt || "image"}…</span>;
    }

    return <span className="md-image-card">
      <button type="button" className="md-image-btn" aria-label={`Enlarge image: ${alt || title || src}`}
        onClick={open}>
        <img ref={imgRef} src={image.displayUrl} alt={alt || ""} title={title || alt || ""}
          className="md-inline-img" onLoad={image.onLoad} onError={image.onError} />
      </button>
      {image.displayKind === "thumbnail" && (
        <span className={`md-image-status${image.status === "full-error" ? " is-error" : ""}`} role="status">
          <span className="pulse-dot" />
          {image.status === "full-error" ? "Full image unavailable" : image.status === "preview" ? "Preview" : "Loading full image…"}
        </span>
      )}
      {image.status === "full-error" && (
        <button type="button" className="md-image-retry" onClick={e => { e.stopPropagation(); image.retry(); }}>Retry full image</button>
      )}
      {(title || alt) && <span className="md-image-caption">{title || alt}</span>}
      {sourceLink}
      {openIndex !== null && <window.ImageLightbox images={[image.displayUrl]} index={openIndex}
        onIndex={setOpenIndex} onClose={close} thumbFor={thumbFor} />}
    </span>;
  }
  window.MarkdownInlineImage = MarkdownInlineImage;
})();
