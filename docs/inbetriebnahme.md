# Inbetriebnahme – Testprotokoll

Schritt-für-Schritt-Prüfung der Steuerung auf dem Raspberry Pi, vom ungefährlichsten zum
kritischsten Schritt. Erst wenn ein Abschnitt vollständig abgehakt ist, mit dem nächsten
weitermachen. Die Einrichtung des Pi (Build, `config.txt`, Installation) steht in der
[readme](../readme.md#raspberry-pi-einrichten-64-bit-raspberry-pi-os).

> ⚠️ **230 V:** Bis einschließlich Abschnitt 8 werden **keine 230-V-Lasten** angeschlossen. Die
> Relaisausgänge nur mit Durchgangsprüfer, Multimeter oder den LEDs des Relaismoduls prüfen.
> Arbeiten an der 230-V-Verdrahtung nur spannungsfrei und nur, wenn du dafür qualifiziert bist.

Werkzeug: Multimeter, Referenzthermometer (am besten mit Fühler), Hygrometer, Föhn oder warme Hand,
optional Logikanalysator/Oszilloskop für die LED-Daten.

Platz für Notizen: Hinter jedem Punkt ist Raum für Messwerte und Auffälligkeiten.

---

## 0. Vor dem ersten Einschalten

- [ ] **DIP-Schalter SW1 auf der PSU-Platine offen.** Er legt +5 V direkt auf GPIO23–25
      (Hardware-Hinweis 1 in der readme). Mit gestecktem Pi nie schließen.
- [ ] I²C-Pull-ups R7/R8 (330 Ω) nicht bestückt oder durch 4,7 kΩ ersetzt (Hardware-Hinweis 2).
- [ ] Relaismodul schaltet mit 3,3-V-Signal und ist aktiv-high (Hardware-Hinweis 4).
- [ ] Netzteil RD-65B: +5 V und +24 V ohne Last gemessen: ______ V / ______ V
- [ ] Polarität der Fühler- und LED-Stecker (J3–J6) gegen den Schaltplan geprüft.

## 1. Pi-Grundeinrichtung

- [ ] 64-Bit Raspberry Pi OS installiert, SSH erreichbar, Hostname: __________
- [ ] Zeitzone: `sudo timedatectl set-timezone Europe/Berlin`, `timedatectl` zeigt die richtige Ortszeit
- [ ] `/boot/firmware/config.txt` wie in der readme ergänzt (inkl. der `gpio=`-Zeilen), neu gestartet
- [ ] `ls /sys/bus/w1/devices/` zeigt drei Einträge `28-…`
- [ ] `i2cdetect -y 1` zeigt `44` (SHT3x) und `UU` auf `68` (DS3231)
- [ ] `sudo hwclock -r` liefert die richtige Zeit

## 2. Installation

- [ ] `docker build --output dist .` am PC, Dateien kopiert, `sudo sh terrarium/deploy/install.sh`
- [ ] In `/opt/terrarium/config.toml`: `simulate = false`, `setup_mode = true`
- [ ] Fühler-IDs in `[sensors]` eingetragen (Zuordnung in Abschnitt 5 prüfen)
- [ ] `sudo systemctl restart terrarium`, `journalctl -u terrarium -n 20` ohne Fehler,
      erste Zeile meldet `hardware: raspberry-pi, setup mode: true`
- [ ] Weboberfläche `http://<pi-ip>/` erreichbar, Dashboard zeigt „mode: setup (schedule paused)“

## 3. Relais beim Einschalten und Booten (ohne Lasten)

Die Relais müssen vom Einstecken des Netzteils bis zum Start des Dienstes **aus** bleiben.
Beobachten mit den LEDs des Relaismoduls oder dem Multimeter an GPIO23/24/25 (Pin 16/18/22) gegen GND.

- [ ] Kalt einschalten (Netzteil einstecken): kein Relais zieht an, auch nicht kurz
- [ ] Während des Bootens bis zum Start des Dienstes: kein Relais zieht an
- [ ] `sudo reboot`: kein Relais zieht während Herunterfahren und Neustart an
- [ ] `sudo systemctl stop terrarium`: alle Relais fallen ab, Log: „stopped, all outputs off“
- [ ] `sudo systemctl start terrarium`: im Inbetriebnahme-Modus bleiben alle Relais aus

## 4. Relaiszuordnung und Zündpause (ohne Lasten)

Im Dashboard (Inbetriebnahme-Modus) gibt es Schaltflächen für UV1, UV2 und Heat.

- [ ] „UV1 on“ → zieht Kanal ______ an. „UV2 on“ → Kanal ______. „Heat on“ → Kanal ______.
- [ ] Falls abweichend von Q1/Q2/Q3 = UV1/UV2/Heat: `[relays]` in `config.toml` anpassen,
      Dienst neu starten, erneut prüfen. Zuordnung in der readme bei „Offene Punkte“ eintragen.
- [ ] „Heat on“ schaltet erst, wenn ein Messwert vom Sonnenplatz vorliegt (sonst „blocked: no basking reading yet“)
- [ ] **Zündpause:** „UV1 on“, dann „UV1 off“, dann sofort wieder „UV1 on“ → UV1 bleibt aus,
      Dashboard zeigt „re-strike pause: 15 min“. Nach Ablauf schaltet UV1 von selbst ein.
- [ ] Dasselbe für UV2
- [ ] Zündpause über Neustart: UV1 an, `sudo systemctl restart terrarium` → nach dem Start
      „UV1 re-strike pause after start“ im Log, UV1 bleibt 15 min aus

## 5. Fühler

- [ ] Zuordnung: einen Fühler in der Hand erwärmen, nur der erwartete Wert steigt
      (Sonnenplatz / kühle Seite / Gehäuse). IDs bei Bedarf in `[sensors]` tauschen.
- [ ] Vergleich mit Referenzthermometer (gleiche Stelle, 10 min warten):
      Sonnenplatz ______ / Ref. ______ °C · kühle Seite ______ / Ref. ______ °C
- [ ] SHT3x gegen Hygrometer: Luft ______ / Ref. ______ °C · Feuchte ______ / Ref. ______ %
- [ ] Gehäusetemperatur (MAX31820) plausibel: ______ °C
- [ ] **Fühlerausfall:** Heat im Inbetriebnahme-Modus an, dann den Sonnenplatz-Fühler abziehen →
      spätestens nach einem Messintervall (60 s) geht Heat aus, Dashboard „basking sensor failed“,
      Log-Eintrag ERROR. Fühler wieder einstecken → Heat darf wieder an.
- [ ] Daten-Seite zeigt die Messwerte des Tages, CSV-Download funktioniert

## 6. Überhitzungsschutz

Zum Testen die Grenze vorübergehend senken, damit sie mit Föhn oder Hand erreichbar ist.

- [ ] In `config.toml`: `overheat_temp = 30.0` (Minimum), `overheat_cooldown_secs = 900`, Dienst neu starten
- [ ] Heat einschalten, Sonnenplatz-Fühler über 30 °C erwärmen → beim nächsten Messwert geht
      Heat aus, Log „OVERHEAT“, Dashboard „blocked: OVERHEAT“
- [ ] Fühler abkühlen lassen → Dashboard zeigt die Sperrzeit („overheat cooldown, … min left“),
      Heat bleibt aus, auch wenn man „Heat on“ drückt
- [ ] Nach 15 min Sperrzeit schaltet Heat wieder ein
- [ ] **Werte zurücksetzen:** `overheat_temp = 50.0`, `overheat_cooldown_secs = 3000`, Dienst neu starten

## 7. LED-Streifen

Testbefehle stehen in der readme („Erster Test mit echter Hardware“). Auf der LED-Seite:
„Override Natural Light“ an, Helligkeit 100 %.

- [ ] Nur Rot (255, 0, 0, 0, 0) → beide Streifen leuchten nur rot
- [ ] Nur Grün, nur Blau, nur Warmweiß, nur Kaltweiß → jeweils der richtige Kanal.
      Falls nicht: `channel_order` in `[led]` anpassen. Ergebnis: `channel_order = "_____"`
- [ ] Alle ICs leuchten gleich (kein „Wandern“ der Farben entlang des Streifens → sonst stimmt die IC-Anzahl oder die Kanalreihenfolge nicht)
- [ ] Kein Flackern, keine zufälligen Farben über 10 min. Sonst `frequency_hz = 1000000` testen,
      LED-Pegel prüfen (Hardware-Hinweis 3). Ergebnis: `frequency_hz = _______`
- [ ] Optional: Timing mit Logikanalysator gemessen: T0H ______ ns, T1H ______ ns
- [ ] Override aus → natürlicher Verlauf, Farbe passt zur Vorschau auf der LED-Seite
- [ ] LED-Leistung aus → beide Streifen dunkel

## 8. Zeitplanbetrieb (ohne Lasten)

- [ ] `setup_mode = false`, Dienst neu starten, Dashboard zeigt „mode: schedule“
- [ ] Für die **aktuelle KW** auf der Wochenplan-Seite Zeiten wenige Minuten in der Zukunft
      eintragen → UV1, UV2, Heat und LEDs schalten zur eingestellten Zeit (Toleranz bis 30 s)
- [ ] Ausschalten zur eingestellten Zeit; UV-Zündpause wird danach angezeigt
- [ ] Die Test-KW wieder auf den Standard zurücksetzen („Reset to default“)
- [ ] Ganze Woche auf „aus“ (Häkchen weg) → Lampen bleiben den ganzen Tag aus (Winterruhe)

## 9. Mit 230-V-Lasten

Zuerst spannungsfrei verdrahten, dann **eine Last nach der anderen**, im Inbetriebnahme-Modus.

- [ ] Heat (Halogenspot): an/aus über das Dashboard, Relais schaltet sauber, auch nach 10× Schalten
      nicht „klebend“ (Einschaltstrom kalter Halogenlampe)
- [ ] UV2 (T5): zündet, geht aus, Zündpause wird eingehalten
- [ ] UV1 (Metalldampflampe am HID-PV C 70): zündet (Aufwärmzeit einige Minuten), geht aus, nach
      15 min Pause zündet sie wieder. **Nicht** heiß wieder einschalten.
- [ ] Alle drei Lasten gleichzeitig an: Pi läuft stabil (keine Neustarts, keine Fehler im Log
      durch Störungen beim Schalten)

## 10. Stromausfall und Neustart

- [ ] UV1 und UV2 brennen, dann **Netzstecker ziehen**, 1 min warten, wieder einstecken →
      nach dem Booten Log „UV1/UV2 re-strike pause after start: 15 min (was on at shutdown/power loss)“,
      UV bleibt 15 min aus, danach schaltet der Zeitplan sie wieder ein
- [ ] Heat bleibt nach dem Start aus, bis der erste Messwert vorliegt
- [ ] **Uhrzeit ohne Netzwerk:** WLAN aus (oder Router aus), Pi neu starten → `date` und Dashboard
      zeigen die richtige Zeit (DS3231), der Zeitplan schaltet zur richtigen Zeit
- [ ] Dienst nach einem Absturz: `sudo systemctl kill -s KILL terrarium` → Dienst ist nach ~10 s
      wieder da (`systemctl status terrarium`), Zündpause greift

## 11. Dauerlauf ohne Tier (24–48 h)

- [ ] `setup_mode = false`, normaler Wochenplan aktiv
- [ ] Temperaturen erreichen die Richtwerte (readme): Sonnenplatz 40–45 °C ______,
      warme Seite 30–35 °C ______, kühle Seite 25–28 °C ______, Nacht 18–22 °C ______
- [ ] Schaltzeiten im Log entsprechen dem Wochenplan, keine unerwarteten Einträge
- [ ] Keine Warnungen außer erwarteten (Daten-Seite, Filter „Warnings“ und „Errors“)
- [ ] Gehäusetemperatur bleibt im Rahmen (Warnschwelle `enclosure_max` = 55 °C): max. ______ °C
- [ ] Wochenplan und Warnschwellen anhand der Messwerte angepasst
- [ ] **Freigabe für den Betrieb mit Tier:** Datum ________ Name ________
