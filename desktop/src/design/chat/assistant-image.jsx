/* Native assistant blocks share the user/tool image viewer. */
(function () {
  function AssistantImage({ image }) {
    return <window.ImageThumbs images={[image]} className="assistant-image-block" alt="Assistant image" />;
  }
  window.AssistantImage = AssistantImage;
})();
