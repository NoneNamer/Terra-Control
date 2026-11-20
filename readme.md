# Terrarium Controller (WIP)

Steuerung für ein Terrarium einer **Bartagame (*Pogona vitticeps*)** auf einem Raspberry Pi 3A+.
Der Controller schaltet UV-Lampen und Wärmespot nach Wochenplan, steuert die LED-Grund- und
Winterbeleuchtung mit tageszeitabhängigem Farbverlauf, misst Temperatur und Luftfeuchtigkeit
und stellt alles über eine lokale Weboberfläche bereit.

> **Stand (2026-11-20):** Die Software läuft am PC mit simulierter Hardware. Die Treiber für den Pi
> (1-Wire, SHT3x, WS2805) sind geschrieben und lassen sich für den Pi bauen, wurden aber noch nicht
> auf dem Pi getestet (Phase 2). Die Platinen sind gefertigt.
> Den aktuellen Arbeitsstand zeigt die [Roadmap](#roadmap).

---

## Funktionen (Version 1)

- **Licht & Wärme**
  - UV1, UV2 und Wärmespot nach **Wochenplan** (ein Eintrag pro Kalenderwoche, KW 1–53)
  - **Zündpause:** UV1 und UV2 dürfen nach dem Ausschalten frühestens nach 15 min wieder einschalten,
    auch über einen Neustart hinweg (siehe unten)
  - **Überhitzungsschutz** für den Wärmespot (Abschaltung plus Sperrzeit)
  - **Sicherer Zustand:** Ohne gültigen Messwert vom Sonnenplatz bleibt der Wärmespot aus
- **LED-Beleuchtung** (Grund- und Winterbeleuchtung)
  - Ein- und Ausschaltzeit plus Grundfarbe pro Kalenderwoche aus dem Wochenplan
  - Innerhalb eines Tages wird der Farbverlauf berechnet (Morgen → Mittag → Abend)
  - Manueller Modus zum Überschreiben der Farbe
- **Messwerte**
  - 2× DS18B20 im Terrarium (Sonnenplatz / kühle Seite)
  - 1× MAX31820 auf der Platine (Temperatur im Gehäuse)
  - 1× SHT3x (Lufttemperatur und Luftfeuchtigkeit, I²C)
  - Speicherung in SQLite, Diagramme und CSV-Export im Web
- **Weboberfläche** – nur im lokalen Netz
- **Echtzeituhr** DS3231 als Zeitquelle, damit die Uhrzeit auch ohne Netzwerk stimmt

**Vorerst gestrichen:** Kamera-Stream, OLED-Display, UV-Messung (VEML6075), Zugriff von außen,
HTTPS, Watchdog. Der alte Kamera-Code (`cam.rs`, `static/cam.html`) wurde in Phase 0 gelöscht
und ist nur noch in der Git-Historie; für Phase 7 wird die Kamera neu umgesetzt.

---

## Getroffene Entscheidungen

| Thema | Entscheidung |
|---|---|
| Datenbank | **SQLite über `sqlx`**, ausschließlich (kein PostgreSQL, kein `rusqlite`). Laufzeit-Abfragen statt `query!`-Makros, Schema als Migrationen in `migrations/` |
| Zeitplan | **Ein Eintrag pro Kalenderwoche**. Berechnet wird nur der Farbverlauf der LEDs innerhalb eines Tages |
| LEDs | Teil der Zeitplan-Logik (Grund- und Winterbeleuchtung), nicht nur manuell |
| GPIO | **Alle GPIO-Zugriffe in `gpio.rs`** hinter dem Trait `Hardware` (Simulation + Pi). Andere Module enthalten nur Logik |
| Inbetriebnahme | `setup_mode = true`: echte Hardware, Zeitplan pausiert, Lampen nur manuell per Web-API schaltbar. Überhitzungsschutz bleibt aktiv |
| Simulation | `[hardware] simulate = true` bzw. immer unter Windows |
| LED-Streifen | Beide Streifen zeigen immer dieselbe Farbe, IC-Anzahl je Streifen konfigurierbar |
| Fühler | **Feste Zuordnung** der 1-Wire-IDs in `config.toml` |
| Luftfeuchtigkeit | **SHT3x** statt DHT22 |
| UV-Messung | Entfällt (Kosten) |
| Kamera / Display | Vorerst gestrichen |
| Netzwerk | Nur lokal, keine Freigabe nach außen. Später ggf. per VPN |
| Relais | Aktiv-high. Verhalten beim Booten muss getestet werden |
| Uhrzeit | DS3231-Echtzeituhr wird genutzt |
| Zielsystem | **64-Bit Raspberry Pi OS** (`aarch64-unknown-linux-gnu`) |
| Build | **Cross-Compiling für den Pi 3A+ in Docker** (`Dockerfile`), Betrieb als **systemd-Dienst** |
| Zündpause nach Neustart | Jeder Schaltvorgang von UV1/UV2 wird gespeichert (UTC). War eine Lampe beim Start laut DB noch **an** (Absturz, Stromausfall), gilt die **volle Pause ab Programmstart**. War sie aus, gilt die Restzeit seit dem Ausschalten |
| Zeitfenster | UV1, UV2 und Wärmespot dürfen **nicht über Mitternacht** laufen (Aus-Zeit vor An-Zeit wird abgelehnt, meist ein Tippfehler). Das LED-Fenster darf über Mitternacht gehen |
| Weboberfläche | Statische Seiten ohne Build-Schritt, gemeinsame Hilfsfunktionen in `static/api.js`. Der Server sendet `Cache-Control: no-cache`, damit nach einem Update keine alten Seiten aus dem Browser-Cache kommen |
| Wärmespot-Sperre | Aus, solange kein Messwert vorliegt, der Fühler ausfällt, der letzte Wert älter als 3 Messintervalle ist, ab `overheat_temp` und während der Sperrzeit. Jeder Wert über der Grenze startet die Sperrzeit neu |
| LED-Treiber | **rpi_ws281x** (über `rs_ws281x`, PWM + DMA auf GPIO18/13). Folge: Programm läuft als root, Analog-Audio aus |
| Abhängigkeiten | Werden auf aktuelle Versionen gehoben (Axum, base64 …) |
| Versionskontrolle | Git |

---

## Hardware

- Raspberry Pi 3A+
- Eigene Platinen (KiCad, gefertigt): `terrarium_pi` (Pi-Aufsatz) und `terrarium_psu` (Verteilerplatine)
- Mean Well RD-65B Netzteil (+24 V / +5 V)
- WS2805 LED-Streifen (RGB + Warmweiß + Kaltweiß, 24 V)
- Relaismodul (Modell noch offen, aktiv-high, ein GPIO pro Relais)

### Lasten

| Kanal | Lampe | Hinweis |
|---|---|---|
| UV1 | Metalldampflampe (MDL) am Philips HID-PV C 70 Vorschaltgerät | Zündpause ≥ 15 min |
| UV2 | T5-Röhre mit Vorschaltgerät in der Fassung | Zündpause ≥ 15 min |
| Heat | 230-V-Halogenspot | Überhitzungsschutz |

### Pinbelegung laut Platine `terrarium_pi`

Ausgelesen aus `terrarium_pi.kicad_pcb` (Netze an J1). Die Nummern sind BCM-GPIO-Nummern.

| GPIO | Pin | Netz / Funktion | Stecker |
|---|---|---|---|
| 2 / 3 | 3 / 5 | I²C SDA / SCL → DS3231 + SHT3x | J9 (GND, SDA, SCL, 3V3) |
| 4 | 7 | `Display_On` (Eingang, gegen 3V3) – Reserve für das OLED-Display (Phase 7) | J10 |
| 5 | 29 | DS3231 RST + Taster SW1 | – |
| 6 | 31 | Status-LED D1 | J8 (externe LED) |
| 8–11 | 24/21/19/23 | SPI0 (MOSI, MISO, SCLK, CE0) für W5500-Ethernet (in v1 nicht genutzt) | J7 (8-pol.: RST, INT, MOSI, MISO, SCLK, SCSn) |
| 22 | 15 | INT von J7 | J7 |
| 27 | 13 | RST von J7 | J7 |
| 17 / 18 | 11 / 12 | LED-Streifen 1: GPIO18 = Daten (DI), GPIO17 = Backup-Daten (BI) | J5 (GND, +24V, GPIO17, GPIO18) |
| 13 / 19 | 33 / 35 | LED-Streifen 2: GPIO13 = Daten (DI), GPIO19 = Backup-Daten (BI) | J6 (GND, +24V, GPIO13, GPIO19) |
| 23 / 24 / 25 | 16 / 18 / 22 | Relais Q3 / Q2 / Q1 | J2 → PSU-Platine J1 → J4 zum Relaisboard |
| 26 | 37 | 1-Wire (4,7k Pull-up, 100R Serie) → MAX31820 + J3/J4 | J3, J4 (DS18B20) |

Die Relais-Leitungen laufen von J2 auf der Pi-Platine zu J1 auf der PSU-Platine: Pi-J2 Pin 4/5/6
(GPIO25/24/23) geht auf PSU-J1 Pin 4/5/6, also **Q1 = GPIO25, Q2 = GPIO24, Q3 = GPIO23**.
Welcher Kanal welche Last schaltet, ist noch offen und wird deshalb in `config.toml` konfigurierbar
gehalten (Vorgabe: Q1 = UV1, Q2 = UV2, Q3 = Heat).

Die Pins in `config.toml` entsprechen seit Phase 1 der Platine: `[relays]` uv1/uv2/heat = 25/24/23,
optionales `led_psu` (auskommentiert), LED-Daten `data_pin_strip1/2` = 18/13. 1-Wire läuft über das
Kernel-Overlay `dtoverlay=w1-gpio,gpiopin=26`, die Fühler werden über ihre festen IDs in `[sensors]`
angesprochen (bis zur Verkabelung Platzhalter).

### Hardware-Hinweise aus der Prüfung

1. **DIP-Schalter SW1 auf der PSU-Platine:** Er legt +5 V direkt auf Q1–Q3, also auf GPIO23–25.
   Die GPIOs des Pi vertragen nur 3,3 V. Ein geschlossener Schalter kann den Pi beschädigen,
   besonders wenn der Pin gleichzeitig Low ausgibt. Mit gestecktem Pi die Schalter nicht benutzen,
   oder eine Entkopplung nachrüsten (Diode oder Serienwiderstand).
2. **I²C-Pull-ups R7/R8 = 330 Ω** sind deutlich zu klein (ca. 10 mA, I²C-Standard max. 3 mA).
   Der Pi hat schon 1,8 kΩ auf GPIO2/3. Empfehlung: R7/R8 nicht bestücken oder 4,7 kΩ einsetzen.
3. **LED-Datenleitungen** gehen ohne Pegelwandler (3,3 V) an die 24-V-WS2805-Streifen.
   Ob das zuverlässig läuft, muss getestet werden.
4. **Relaismodul mit 5 V Versorgung** an 3,3-V-Steuersignalen: Das neue Modul muss mit 3,3 V
   High-Pegel sicher schalten.
5. **Verhalten beim Booten:** GPIO23–25 sind beim Start Eingänge mit Pull-down. Bei aktiv-high
   Relais sollten sie aus bleiben. Das muss getestet werden.

---

## Raspberry Pi einrichten (64-Bit Raspberry Pi OS)

### Programm bauen und kopieren

Gebaut wird am PC in Docker (Cross-Compiling für `aarch64`):

```bash
docker build --output dist .
```

Danach liegt das Programm in `dist/terra-control`. Auf den Pi gehören drei Dinge in einen Ordner,
z. B. `/opt/terrarium`: `terra-control`, `config.toml` und der Ordner `static/`.

```bash
scp -r dist/terra-control config.toml static <user>@<pi-ip>:/opt/terrarium/
```

Das Programm muss als **root** laufen: Der LED-Treiber (rpi_ws281x) braucht `/dev/mem`, und Port 80
ist ein privilegierter Port.

```bash
cd /opt/terrarium && sudo ./terra-control
```

`data.db`, `logs/` und `temp/` entstehen im aktuellen Ordner. Ein systemd-Dienst folgt in Phase 5.

### `/boot/firmware/config.txt`

```ini
dtparam=i2c_arm=on              # I²C für DS3231 und SHT3x
dtoverlay=w1-gpio,gpiopin=26    # 1-Wire auf GPIO26
dtoverlay=i2c-rtc,ds3231        # Echtzeituhr
dtparam=audio=off               # PWM wird für die LED-Streifen gebraucht
```

SPI muss nicht aktiviert werden (J7 bzw. das W5500 wird in v1 nicht genutzt). Danach neu starten.

### Prüfen

- **1-Wire:** `ls /sys/bus/w1/devices/` zeigt drei Einträge `28-…` (DS18B20 und MAX31820 haben
  denselben Family-Code). Zuordnen lassen sie sich, indem man einen Fühler in der Hand erwärmt und
  `cat /sys/bus/w1/devices/28-…/temperature` vergleicht. Die IDs kommen nach `[sensors]` in `config.toml`.
- **I²C:** `sudo apt install i2c-tools`, dann `i2cdetect -y 1`. Erwartet: `44` (SHT3x; `45`, wenn
  ADDR auf High liegt) und `UU` auf `68` (DS3231, vom Kernel-Treiber belegt).
- **Echtzeituhr:** `sudo hwclock -r` liest die DS3231. Damit sie beim Booten die Systemzeit setzt,
  `fake-hwclock` entfernen (`sudo apt remove fake-hwclock`) und einmal bei funktionierendem NTP
  `sudo hwclock -w` ausführen. Ob die Zeit nach einem Neustart ohne Netzwerk stimmt, muss getestet
  werden (unter Bookworm verhindert ggf. eine Abfrage auf systemd in `/lib/udev/hwclock-set` das
  Stellen der Uhr – dann diese Zeilen auskommentieren).

### Erster Test mit echter Hardware

In `config.toml`: `simulate = false` und `setup_mode = true`. Im Inbetriebnahme-Modus schaltet der
Zeitplan nichts; die Lampen lassen sich nur per API schalten, der Überhitzungsschutz bleibt aktiv.
Für die ersten Tests die 230-V-Lasten abklemmen.

```bash
curl http://<pi-ip>/api/values                       # Messwerte und Relaiszustände
curl -X POST -H 'Content-Type: application/json' -d '{"on":true}' http://<pi-ip>/api/relays/uv1
```

**LED-Streifen:** Im Inbetriebnahme-Modus zeigen die LEDs die Farbe sofort, unabhängig vom
Zeitfenster. Mit manueller Farbe und 100 % Helligkeit jeden Kanal einzeln testen:

```bash
curl -X POST -H 'Content-Type: application/json' -d '{"override_settings":true,"season_weight":0.3}' http://<pi-ip>/api/led/natural
curl -X POST -H 'Content-Type: application/json' -d '{"brightness":100}' http://<pi-ip>/api/led/brightness
curl -X POST -H 'Content-Type: application/json' -d '{"r":255,"g":0,"b":0,"ww":0,"cw":0}' http://<pi-ip>/api/led/color
```

- Leuchtet ein falscher Kanal, `channel_order` in `[led]` anpassen (Reihenfolge der 5 Bytes je IC,
  `w` = warmweiß, `c` = kaltweiß).
- **Timing:** rpi_ws281x kodiert jedes Bit mit drei PWM-Takten. Bei `frequency_hz = 800000` ist
  eine 0 etwa 417 ns und eine 1 etwa 833 ns lang (High-Zeit). Laut dem alten Code dieses Projekts
  erwartet der WS2805 eher ~300 ns für eine 0. Wenn Farben falsch oder zu hell sind oder flackern,
  `frequency_hz = 1000000` (333/667 ns) probieren und mit dem Datenblatt abgleichen.
- Die Datenleitungen haben keinen Pegelwandler (3,3 V an 24-V-Streifen, siehe Hardware-Hinweis 3).

---

## Richtwerte Bartagame (Ausgangswerte, vor Nutzung prüfen)

| Parameter | Richtwert |
|---|---|
| Sonnenplatz (Boden) | 40–45 °C, lokal bis ca. 50 °C |
| Warme Seite (Luft) | 30–35 °C |
| Kühle Seite | 25–28 °C |
| Nacht | 18–22 °C |
| Luftfeuchtigkeit | 30–40 % (Nacht etwas höher) |
| Beleuchtungsdauer | Sommer 12–14 h, Winter 8–10 h |
| Winterruhe | ca. 6–10 Wochen bei 15–18 °C, UV und Wärmespot aus, nur LED-Grundlicht |

Diese Werte dienen als Standard für `config.toml`, für die Wochenplan-Vorlage und für die Warnschwellen.

---

## Projektstruktur

```
├── main.rs              # Einstiegspunkt, startet Tasks, schaltet beim Beenden alles aus
├── config.toml          # Konfiguration
├── Cargo.toml           # Abhängigkeiten
├── Dockerfile           # Cross-Compiling für den Pi (aarch64), Ausgabe nach dist/
├── migrations/          # SQLite-Schema (sqlx-Migrationen, beim Start automatisch angewendet)
├── modules/
│   ├── mod.rs           # Modul-Einstieg
│   ├── models.rs        # Datenmodelle (Farbe, Zeitfenster, Wochenplan, Messwerte)
│   ├── state.rs         # Gemeinsamer Zustand für Tasks und Web-Handler
│   ├── config.rs        # Laden und Prüfen von config.toml
│   ├── gpio.rs          # SÄMTLICHER Hardware-Zugriff: Trait `Hardware`, Simulation, Pi-Backend
│   ├── getData.rs       # Messwerterfassung (nutzt gpio.rs)
│   ├── storage.rs       # SQLite (sqlx), Schema
│   ├── lightControl.rs  # Logik UV / Wärmespot / Zündpause / Überhitzung
│   ├── ledStrip.rs      # Logik LED-Verlauf
│   ├── logs.rs          # Logging
│   └── web.rs           # Web-API (Axum)
├── static/              # Weboberfläche
│   ├── index.html       # Dashboard
│   ├── schedule.html    # Wochenplan
│   ├── data.html        # Logs & Daten
│   ├── led.html         # LED-Steuerung
│   ├── api.js
│   └── styles.css
└── logs/                # Log-Dateien (zur Laufzeit)
```

---

## Roadmap

### Phase 0 – Aufräumen
- [x] Git-Repository anlegen
- [x] `cargo.toml` → `Cargo.toml`
- [x] Abhängigkeiten aktualisieren (axum 0.8, sqlx 0.9, toml 1.x, tower-http 0.7, rppal 0.22, zip 9).
      Entfernt: `rusqlite`, `dht-sensor`, `raspicam`, `image`, `base64`, `embedded-hal`, `bytes`, `tokio-stream`.
      `rppal` nur noch für Linux-Ziele
- [x] Ungenutzte Module aus `mod.rs` nehmen (`display`, `cam`), `gpio` ergänzt
- [x] Kamera-Code gelöscht (`cam.rs`, `cam.html`, Kamera-Server in `main.rs`, Kamera-Routen in `web.rs`)
- [x] `Cargo.lock` wird versioniert (Binärprojekt), `data.db` und `temp/` ignoriert

### Phase 1 – Kompilierbar machen
- [x] `config.rs`/`config.toml` vereinheitlichen, Pins laut Platine korrigieren (unbekannte Schlüssel sind ein Fehler)
- [x] Fühler-IDs (1-Wire) fest in `config.toml` (noch Platzhalter)
- [x] Ein SQLite-Schema mit `sqlx`-Migrationen (Wochenplan inkl. LED-Zeiten und -Farbe, Messwerte, Logs, LED-Einstellungen, LED-Presets)
- [x] `gpio.rs` als einzige Hardware-Schicht: Relais, LED-Streifen, 1-Wire, I²C
- [x] Hardware hinter einem Trait mit **Simulation** (einfaches Wärmemodell), läuft unter Windows
- [x] Relais-Zuordnung Q1–Q3 → UV1/UV2/Heat konfigurierbar, optionales LED-Netzteil-Relais
- [x] `static/` über den Webserver ausliefern
- [x] Module verdrahten, fehlende Typen ergänzen; Inbetriebnahme-Modus (`setup_mode`) mit `POST /api/relays/{uv1|uv2|heat}`
- [x] Dashboard (`index.html`) an die neuen Messwerte angepasst
- [x] Pi-Backend (`#[cfg(target_os = "linux")]`) kompiliert (Docker-Build, Phase 2)
- **Ziel:** `cargo build` läuft, die Weboberfläche startet am PC mit simulierten Werten

### Phase 2 – Sensoren & LED-Treiber
- [x] DS18B20 / MAX31820 über 1-Wire (sysfs `temperature`, Rückfall auf `w1_slave` mit CRC-Prüfung)
- [x] SHT3x über I²C (Einzelmessung, CRC-Prüfung)
- [x] DS3231 als Systemzeit – Einrichtung beschrieben (kein Code nötig)
- [x] WS2805-Treiber für GPIO18/13: rpi_ws281x, 5 Bytes je IC als RGB-Bytestrom, Kanalreihenfolge und Frequenz konfigurierbar, BI-Leitungen (GPIO17/19) auf Low
- [x] Pi-Backend baut im Docker (Cross-Compiling vorgezogen aus Phase 5)
- [ ] **Am Pi testen** (siehe „Raspberry Pi einrichten“): Fühler-IDs eintragen, Kanalreihenfolge und Timing der LEDs prüfen, RTC ohne Netzwerk
- **Ziel:** echte Messwerte und LED-Ausgabe auf dem Pi

### Phase 3 – Sicherheit & Logik
- [x] Zündpause UV1/UV2 (≥ 15 min nach dem Ausschalten, auch nach einem Neustart; Tabelle `lamp_state`)
- [x] Überhitzungsschutz Wärmespot getestet (Unit-Tests: Abschaltung, Sperrzeit, Neustart der Sperrzeit)
- [x] Sicherer Zustand bei Sensorausfall (Wärmespot aus; auch bei veraltetem Messwert)
- [x] Unit-Tests für Zeitplan, Zündpause, Überhitzung und LED-Verlauf (26 Tests, per eingebautem Fehler gegengeprüft)
- [x] Zündpause und Sperrgrund im Dashboard bzw. in `/api/values` und `/api/system/status`
- [ ] Am Pi: Überhitzungsschutz und Zündpause mit echter Hardware nachprüfen (Phase 6)
- **Ziel:** Der Wärmespot schaltet nachweisbar sicher

### Phase 4 – Weboberfläche
- [x] API-Routen und Frontend abgleichen (`api.js` als gemeinsame Schicht, alle Seiten nutzen die aktuelle API)
- [x] Wochenplan-Seite (KW 1–53) inkl. LED-Zeiten und -Farbe: Übersicht aller Wochen, Editor, Übertragen auf einen KW-Bereich, Zurücksetzen auf den Standard
- [x] Eingaben serverseitig prüfen (einheitliche JSON-Fehler, Wochen 1–53, Zeiten, Farbwerte, kein UV/Wärme über Mitternacht, Datumsbereiche)
- [x] Dashboard: Sperrgrund Wärmespot, Zündpause, Systemstatus; im Inbetriebnahme-Modus Schaltflächen für UV1/UV2/Heat
- [x] LED-Seite: Override-Schalter war vertauscht (behoben), Helligkeitsregler, Vorschau nach dem echten LED-Fenster der Woche
- [x] Daten-Seite: lesbar im dunklen Design, Messwerte eines Tages als Tabelle
- [ ] Chart.js wird vom CDN geladen – ohne Internet am Client fehlen nur die Diagramme (ggf. lokal ablegen)
- **Ziel:** komplett im Browser bedienbar

### Phase 5 – Deployment
- [x] Dockerfile zum Cross-Compiling für den Pi 3A+ (in Phase 2 vorgezogen)
- [ ] systemd-Dienst mit automatischem Neustart
- [ ] Log-Rotation / Datenbereinigung (`storage_days`)
- **Ziel:** läuft dauerhaft auf dem Pi

### Phase 6 – Hardware & Inbetriebnahme
- [ ] Relaismodul auswählen (3 Kanäle, schaltet mit 3,3 V)
- [ ] Hardware-Hinweise oben klären (DIP-Schalter, I²C-Pull-ups, LED-Pegel)
- [ ] Platinen bestücken, Netzteil-Box, Verkabelung
- [ ] Halterungen für die Fühler, Deckel
- [ ] Relais-Verhalten beim Booten testen
- [ ] Testlauf am Terrarium
- **Ziel:** Betrieb am Terrarium

### Phase 7 – Später (optional)
- Kamera (z. B. MJPEG über `rpicam-vid`)
- OLED-Display (`Display_On` an GPIO4 ist schon vorhanden)
- Zugriff von außen per VPN
- Watchdog, strukturiertes Logging (`tracing`)

---

## Offene Punkte

- [ ] Zuordnung Q1/Q2/Q3 → UV1/UV2/Heat bestätigen (bis dahin konfigurierbar, Vorgabe Q1/Q2/Q3 = UV1/UV2/Heat)
- [ ] Pin für das nachzurüstende LED-Netzteil-Relais
- [x] J7 ist für ein **W5500-Ethernet-Modul** (in v1 nicht genutzt)
- [x] Zwei Datenleitungen pro LED-Streifen = **Daten (DI) + Backup-Daten (BI)**
- [x] `Display_On` (GPIO4) bleibt **Reserve für das OLED-Display** (Phase 7)
- [x] LED-Netzteil-Relais wird **später nachgerüstet** → im Code optional
- [x] **64-Bit** Raspberry Pi OS → Cross-Compile-Ziel `aarch64-unknown-linux-gnu`

---

## Entwicklung

- Rust, Tokio, Axum
- SQLite über `sqlx`
- `rppal` für GPIO / I²C / SPI auf dem Pi

Die Weboberfläche ist erreichbar unter `http://<pi-ip>:80` (nur im lokalen Netz).
