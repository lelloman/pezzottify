#!/bin/bash
#
# Validate media files using ffprobe
#
# Usage:
#   ./validate_media.sh /path/to/media          # Check audio and images
#   ./validate_media.sh /path/to/media audio    # Check audio only
#   ./validate_media.sh /path/to/media images   # Check images only
#   ./validate_media.sh /path/to/media --delete # Delete corrupted files
#
# Examples:
#   ./validate_media.sh /data/media
#   ./validate_media.sh /data/media audio --delete
#

set -e

MEDIA_PATH="${1:-.}"
TYPE="${2:-all}"
DELETE_CORRUPTED=false

# Check for --delete flag in any position
for arg in "$@"; do
    if [ "$arg" = "--delete" ]; then
        DELETE_CORRUPTED=true
    fi
done

# Ensure ffprobe is available
if ! command -v ffprobe &> /dev/null; then
    echo "Error: ffprobe not found. Install ffmpeg package."
    exit 1
fi

CORRUPTED_COUNT=0
VALID_COUNT=0
TOTAL_COUNT=0

validate_file() {
    local file="$1"
    local type="$2"

    TOTAL_COUNT=$((TOTAL_COUNT + 1))

    # Run ffprobe and capture stderr
    if ffprobe -v error -show_entries format=duration -of csv=p=0 "$file" > /dev/null 2>&1; then
        VALID_COUNT=$((VALID_COUNT + 1))
    else
        CORRUPTED_COUNT=$((CORRUPTED_COUNT + 1))
        echo "CORRUPTED: $file"

        if [ "$DELETE_CORRUPTED" = true ]; then
            rm "$file"
            echo "  -> Deleted"
        fi
    fi

    # Progress indicator every 100 files
    if [ $((TOTAL_COUNT % 100)) -eq 0 ]; then
        echo "Progress: $TOTAL_COUNT files checked ($CORRUPTED_COUNT corrupted)" >&2
    fi
}

echo "Validating media files in: $MEDIA_PATH"
echo "Type: $TYPE"
echo "Delete corrupted: $DELETE_CORRUPTED"
echo "---"

# Validate audio files
if [ "$TYPE" = "all" ] || [ "$TYPE" = "audio" ]; then
    AUDIO_PATH="$MEDIA_PATH/audio"
    if [ -d "$AUDIO_PATH" ]; then
        echo "Scanning audio files..."
        while IFS= read -r -d '' file; do
            validate_file "$file" "audio"
        done < <(find "$AUDIO_PATH" -type f \( -name "*.ogg" -o -name "*.flac" -o -name "*.mp3" -o -name "*.m4a" -o -name "*.aac" -o -name "*.wav" \) -print0)
    else
        echo "Audio directory not found: $AUDIO_PATH"
    fi
fi

# Validate image files
if [ "$TYPE" = "all" ] || [ "$TYPE" = "images" ]; then
    IMAGES_PATH="$MEDIA_PATH/images"
    if [ -d "$IMAGES_PATH" ]; then
        echo "Scanning image files..."
        while IFS= read -r -d '' file; do
            validate_file "$file" "image"
        done < <(find "$IMAGES_PATH" -type f \( -name "*.jpg" -o -name "*.jpeg" -o -name "*.png" -o -name "*.webp" \) -print0)
    else
        echo "Images directory not found: $IMAGES_PATH"
    fi
fi

echo "---"
echo "Validation complete!"
echo "Total files: $TOTAL_COUNT"
echo "Valid: $VALID_COUNT"
echo "Corrupted: $CORRUPTED_COUNT"

if [ $CORRUPTED_COUNT -gt 0 ]; then
    exit 1
fi
