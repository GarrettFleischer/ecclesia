function inviteCode(raw) {
  var value = String(raw || "").trim();
  try {
    var url = new URL(value);
    var parts = url.pathname.split("/").filter(Boolean);
    if (parts[0] === "join" && parts.length > 1) {
      return decodeURIComponent(parts.slice(1).join("/"));
    }
  } catch (_error) {
    return value;
  }
  return value;
}

(function () {
  var params = new URLSearchParams(window.location.search);
  if (!params.has("lat") && navigator.geolocation) {
    navigator.geolocation.getCurrentPosition(function (pos) {
      params.set("lat", String(pos.coords.latitude));
      params.set("lng", String(pos.coords.longitude));
      window.location.search = params.toString();
    });
  }

  var button = document.querySelector("[data-scan-code]");
  var sheet = document.querySelector("[data-scan-sheet]");
  var video = document.querySelector("[data-scan-video]");
  var note = document.querySelector("[data-scan-status]");
  var close = document.querySelector("[data-scan-close]");
  if (!button || !sheet || !video) {
    return;
  }

  var stream = null;

  function say(text) {
    if (note) {
      note.textContent = text;
    }
  }

  function stop() {
    if (stream) {
      stream.getTracks().forEach(function (track) {
        track.stop();
      });
      stream = null;
    }
    video.srcObject = null;
    sheet.hidden = true;
  }

  if (close) {
    close.addEventListener("click", function () {
      say("");
      stop();
    });
  }

  button.addEventListener("click", function () {
    say("");
    if (!("BarcodeDetector" in window) || !navigator.mediaDevices) {
      say("The camera didn't open. Try again.");
      return;
    }
    var detector = new BarcodeDetector({ formats: ["qr_code"] });
    navigator.mediaDevices
      .getUserMedia({ video: { facingMode: "environment" } })
      .then(function (opened) {
        stream = opened;
        video.srcObject = opened;
        sheet.hidden = false;
        video.play();
        var tick = function () {
          if (!stream) {
            return;
          }
          detector
            .detect(video)
            .then(function (codes) {
              if (codes.length > 0) {
                var code = inviteCode(codes[0].rawValue);
                stop();
                if (code) {
                  window.location.assign("/join/" + encodeURIComponent(code));
                }
                return;
              }
              requestAnimationFrame(tick);
            })
            .catch(stop);
        };
        video.addEventListener("loadeddata", tick, { once: true });
      })
      .catch(function () {
        say("The camera didn't open. Try again.");
        stop();
      });
  });
})();
