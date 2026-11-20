# Terrarium Controller (WIP)

Steuerung für ein Terrarium einer **Bartagame (*Pogona vitticeps*)** auf einem Raspberry Pi 3A+.
Der Controller schaltet UV-Lampen und Wärmespot nach Wochenplan, steuert die LED-Grund- und
Winterbeleuchtung mit tageszeitabhängigem Farbverlauf, misst Temperatur und Luftfeuchtigkeit
und stellt alles über eine lokale Weboberfläche bereit.

> **Stand (2026-11-20):** Die Software kompiliert noch nicht. Die Platinen sind gefertigt.
> Den aktuellen Arbeitsstand zeigt die [Roadmap](#roadmap).

---

## Funktionen (Version 1)

- **Licht & Wärme**
  - UV1, UV2 und Wärmespot nach **Wochenplan** (ein Eintrag pro Kalenderwoche, KW 1–53)
  - **Zündpause:** UV1 und UV2 dürfen nach dem Ausschalten frühestens nach 15 min wieder einschalten
  - **Überhitzungsschutz** für den Wärmespot (Abschaltung plus Sperrzeit)
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
| Datenbank | **SQLite über `sqlx`**, ausschließlich (kein PostgreSQL, kein `rusqlite`) |
| Zeitplan | **Ein Eintrag pro Kalenderwoche**. Berechnet wird nur der Farbverlauf der LEDs innerhalb eines Tages |
| LEDs | Teil der Zeitplan-Logik (Grund- und Winterbeleuchtung), nicht nur manuell |
| GPIO | **Alle GPIO-Zugriffe in `gpio.rs`**. Andere Module enthalten nur Logik und rufen `gpio.rs` auf |
| Fühler | **Feste Zuordnung** der 1-Wire-IDs in `config.toml` |
| Luftfeuchtigkeit | **SHT3x** statt DHT22 |
| UV-Messung | Entfällt (Kosten) |
| Kamera / Display | Vorerst gestrichen |
| Netzwerk | Nur lokal, keine Freigabe nach außen. Später ggf. per VPN |
| Relais | Aktiv-high. Verhalten beim Booten muss getestet werden |
| Uhrzeit | DS3231-Echtzeituhr wird genutzt |
| Zielsystem | **64-Bit Raspberry Pi OS** (`aarch64-unknown-linux-gnu`) |
| Build | **Cross-Compiling für den Pi 3A+ in Docker**, Betrieb als **systemd-Dienst** |
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

> ⚠️ **Die Pins in `config.toml` passen nicht zur Platine** und werden in Phase 1 angepasst:
>
> | Eintrag | config.toml | Platine |
> |---|---|---|
> | `uv_relay1` | 22 | 25 (Q1) – Zuordnung bestätigen |
> | `uv_relay2` | 23 | 24 (Q2) – Zuordnung bestätigen |
> | `heat_relay` | 27 | 23 (Q3) – Zuordnung bestätigen |
> | `led_relay` | 17 | **kein Relais auf der Platine** (nur 3 Kanäle), wird später nachgerüstet → optional, Pin offen. GPIO17 ist LED-Streifen 1 (BI) |
> | `ds18b20_bus` | 4 | **26** (`dtoverlay=w1-gpio,gpiopin=26`) |
> | `dht22_pin` | 18 | entfällt (SHT3x über I²C). GPIO18 ist LED-Streifen 1 |
> | LED-Daten | SPI0 MOSI (GPIO10) im Code | GPIO18/13 (PWM). SPI0 ist an J7 belegt |

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
├── main.rs              # Einstiegspunkt, startet Tasks
├── config.toml          # Konfiguration
├── Cargo.toml           # Abhängigkeiten
├── modules/
│   ├── mod.rs           # Modul-Einstieg
│   ├── models.rs        # Datenmodelle
│   ├── config.rs        # Laden und Prüfen von config.toml
│   ├── gpio.rs          # SÄMTLICHER Hardware-Zugriff (Relais, LED-Streifen, Sensoren)
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
- [ ] `config.rs`/`config.toml` vereinheitlichen, Pins laut Tabelle oben korrigieren
- [ ] Fühler-IDs (1-Wire) fest in `config.toml`
- [ ] Ein SQLite-Schema mit `sqlx`-Migrationen (Wochenplan inkl. LED-Zeiten und -Farbe, Messwerte, Logs, LED-Einstellungen)
- [ ] `gpio.rs` als einzige Hardware-Schicht: Relais, LED-Streifen, 1-Wire, I²C
- [ ] Hardware hinter einem Trait mit **Simulation**, damit das Programm auch unter Windows läuft
- [ ] Relais-Zuordnung Q1–Q3 → UV1/UV2/Heat konfigurierbar, optionales LED-Netzteil-Relais
- [ ] `static/` über den Webserver ausliefern (bisher nur über den gestrichenen Kamera-Server)
- [ ] Module verdrahten, fehlende Typen ergänzen
- **Ziel:** `cargo build` läuft, die Weboberfläche startet am PC mit simulierten Werten

### Phase 2 – Sensoren & LED-Treiber
- [ ] DS18B20 / MAX31820 über 1-Wire (sysfs, `w1-gpio` auf GPIO26)
- [ ] SHT3x über I²C
- [ ] DS3231 als Systemzeit (`dtoverlay=i2c-rtc,ds3231`)
- [ ] WS2805-Treiber für GPIO18/13 (PWM) klären und umsetzen
- **Ziel:** echte Messwerte und LED-Ausgabe auf dem Pi

### Phase 3 – Sicherheit & Logik
- [ ] Zündpause UV1/UV2 (≥ 15 min nach dem Ausschalten, auch nach einem Neustart)
- [ ] Überhitzungsschutz Wärmespot testen
- [ ] Sicherer Zustand bei Sensorausfall (Wärmespot aus)
- [ ] Unit-Tests für Zeitplan, Zündpause und LED-Verlauf
- **Ziel:** Der Wärmespot schaltet nachweisbar sicher

### Phase 4 – Weboberfläche
- [ ] API-Routen und Frontend abgleichen (`api.js` ↔ `web.rs`)
- [ ] Wochenplan-Seite (KW 1–53) fertigstellen, inkl. LED-Zeiten und -Farbe
- [ ] Eingaben serverseitig prüfen
- **Ziel:** komplett im Browser bedienbar

### Phase 5 – Deployment
- [ ] Dockerfile zum Cross-Compiling für den Pi 3A+
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
