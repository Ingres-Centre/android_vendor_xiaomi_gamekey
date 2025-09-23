package org.ingres.gamekeys;

import org.ingres.gamekeys.Point;

@VintfStability
interface ISettingsService {
    void setSettings(in @nullable Point upper, in @nullable Point lower);
}
