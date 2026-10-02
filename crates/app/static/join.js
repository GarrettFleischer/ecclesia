(function () {
  var params = new URLSearchParams(window.location.search);
  if (!params.has("lat") && navigator.geolocation) {
    navigator.geolocation.getCurrentPosition(function (pos) {
      params.set("lat", String(pos.coords.latitude));
      params.set("lng", String(pos.coords.longitude));
      window.location.search = params.toString();
    });
  }

  var field = document.querySelector("[data-invite-code]");
  var button = document.querySelector("[data-scan-code]");
  if (!field || !button || !("BarcodeDetector" in window) || !navigator.mediaDevices) {
    return;
  }
  button.hidden = false;
  button.addEventListener("click", function () {
    var detector = new BarcodeDetector({ formats: ["qr_code"] });
    navigator.mediaDevices
      .getUserMedia({ video: { facingMode: "environment" } })
      .then(function (stream) {
        var video = document.createElement("video");
        video.srcObject = stream;
        video.playsInline = true;
        video.play();
        var stop = function () {
          stream.getTracks().forEach(function (track) {
            track.stop();
          });
        };
        var tick = function () {
          detector
            .detect(video)
            .then(function (codes) {
              if (codes.length > 0) {
                field.value = codes[0].rawValue;
                stop();
                return;
              }
              requestAnimationFrame(tick);
            })
            .catch(stop);
        };
        video.addEventListener("loadeddata", tick);
      });
  });
})();
