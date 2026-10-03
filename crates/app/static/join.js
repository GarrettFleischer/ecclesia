/**
 * @fileoverview Find a church from the device location, a name, or a city, and scan a church code.
 * @module join
 */

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
  var form = document.querySelector("[data-join-finder]");
  var box = document.querySelector("[data-join-query]");
  var pause = null;
  var flight = null;
  var generation = 0;
  var pauseMs = 300;

  function fieldValue(name) {
    if (!form) {
      return "";
    }
    var field = form.querySelector("[name='" + name + "']");
    return field ? field.value.trim() : "";
  }

  function finderUrl() {
    var params = new URLSearchParams();
    var lat = fieldValue("lat");
    var lng = fieldValue("lng");
    var q = box ? box.value.trim() : "";
    if (lat && lng) {
      params.set("lat", lat);
      params.set("lng", lng);
    }
    if (q) {
      params.set("q", q);
    }
    var query = params.toString();
    return query ? "/churches/join?" + query : "/churches/join";
  }

  function paintFailure() {
    var current = document.querySelector("[data-join-results]");
    if (current) {
      current.textContent = "The list didn't load. Try again.";
    }
  }

  function run() {
    generation += 1;
    var ticket = generation;
    if (flight) {
      flight.abort();
    }
    flight = new AbortController();
    var url = finderUrl();
    fetch(url, { signal: flight.signal, headers: { Accept: "text/html" } })
      .then(function (response) {
        if (!response.ok) {
          throw new Error("list");
        }
        return response.text();
      })
      .then(function (html) {
        if (ticket !== generation) {
          return;
        }
        var doc = new DOMParser().parseFromString(html, "text/html");
        var next = doc.querySelector("[data-join-results]");
        var current = document.querySelector("[data-join-results]");
        if (!next || !current) {
          return;
        }
        current.replaceWith(next);
        if (window.location.pathname + window.location.search !== url) {
          history.replaceState(null, "", url);
        }
      })
      .catch(function (error) {
        if (error && error.name === "AbortError") {
          return;
        }
        if (ticket !== generation) {
          return;
        }
        paintFailure();
      });
  }

  function schedule(immediate) {
    window.clearTimeout(pause);
    pause = window.setTimeout(run, immediate ? 0 : pauseMs);
  }

  function rememberPlace(latitude, longitude) {
    var latField = form && form.querySelector("[name='lat']");
    var lngField = form && form.querySelector("[name='lng']");
    if (latField) {
      latField.value = String(latitude);
    }
    if (lngField) {
      lngField.value = String(longitude);
    }
    schedule(true);
  }

  /**
   * @brief Read a position the browser can already share.
   */
  function requestPlace() {
    navigator.geolocation.getCurrentPosition(
      function (pos) {
        rememberPlace(pos.coords.latitude, pos.coords.longitude);
      },
      function () {},
      { enableHighAccuracy: false, maximumAge: 300000, timeout: 10000 }
    );
  }

  /**
   * @brief Ask for location when it is not already granted, then list nearby churches.
   * @returns {void}
   */
  function askForPlace() {
    if (!navigator.geolocation || (fieldValue("lat") && fieldValue("lng"))) {
      return;
    }
    if (!navigator.permissions || !navigator.permissions.query) {
      requestPlace();
      return;
    }
    navigator.permissions
      .query({ name: "geolocation" })
      .then(function (status) {
        if (status.state === "denied") {
          return;
        }
        requestPlace();
      })
      .catch(requestPlace);
  }

  askForPlace();

  if (form) {
    form.addEventListener("submit", function (event) {
      event.preventDefault();
      schedule(true);
    });
  }

  if (box) {
    box.addEventListener("input", function () {
      schedule(false);
    });
    box.addEventListener("search", function () {
      schedule(true);
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
