import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import androidx.palette.graphics.Palette;
import org.json.JSONArray;
import org.json.JSONObject;

/** Run on Android via app_process with AndroidX Palette 1.0.0 on the classpath. */
public final class PaletteStudy {
    private static Object swatch(Palette.Swatch value) throws Exception {
        if (value == null) return JSONObject.NULL;
        return new JSONObject().put("color", String.format("#%06X", value.getRgb() & 0xffffff))
            .put("population", value.getPopulation());
    }

    public static void main(String[] args) throws Exception {
        JSONArray results = new JSONArray();
        for (String path : args) {
            Bitmap bitmap = BitmapFactory.decodeFile(path);
            if (bitmap == null) throw new IllegalArgumentException("Cannot decode " + path);
            Palette palette = Palette.from(bitmap).generate();
            JSONObject profiles = new JSONObject()
                .put("Dominant", swatch(palette.getDominantSwatch()))
                .put("Vibrant", swatch(palette.getVibrantSwatch()))
                .put("Light vibrant", swatch(palette.getLightVibrantSwatch()))
                .put("Dark vibrant", swatch(palette.getDarkVibrantSwatch()))
                .put("Muted", swatch(palette.getMutedSwatch()))
                .put("Light muted", swatch(palette.getLightMutedSwatch()))
                .put("Dark muted", swatch(palette.getDarkMutedSwatch()));
            JSONArray swatches = new JSONArray();
            for (Palette.Swatch s : palette.getSwatches()) swatches.put(swatch(s));
            JSONObject result = new JSONObject().put("file", new java.io.File(path).getName())
                .put("profiles", profiles).put("swatches", swatches);
            // Optional parity fixture: quantized input histogram before filtering.
            // Feeding this back to the port isolates quantization from JPEG decoding.
            if (Boolean.getBoolean("palette.fixtures")) {
                double ratio = Math.min(1, Math.sqrt(12544.0 / (bitmap.getWidth() * bitmap.getHeight())));
                Bitmap scaled = Bitmap.createScaledBitmap(bitmap,
                    (int)Math.ceil(bitmap.getWidth() * ratio),
                    (int)Math.ceil(bitmap.getHeight() * ratio), false);
                int[] pixels = new int[scaled.getWidth() * scaled.getHeight()];
                scaled.getPixels(pixels, 0, scaled.getWidth(), 0, 0, scaled.getWidth(), scaled.getHeight());
                int[] histogram = new int[32768];
                for (int c : pixels) histogram[((c >> 19) & 31) << 10 | ((c >> 11) & 31) << 5 | ((c >> 3) & 31)]++;
                JSONArray counts = new JSONArray();
                for (int c = 0; c < histogram.length; c++) {
                    if (histogram[c] > 0) counts.put(new JSONArray().put(c).put(histogram[c]));
                }
                result.put("histogram", counts);
                if (scaled != bitmap) scaled.recycle();
            }
            results.put(result);
            bitmap.recycle();
        }
        System.out.println(new JSONObject().put("library", "androidx.palette:palette:1.0.0")
            .put("settings", "Default builder: 16 colors, 12544-pixel resize area, default filter and targets")
            .put("android_api", android.os.Build.VERSION.SDK_INT).put("samples", results));
    }
}
