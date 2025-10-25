/**
 * Analytics utility for tracking data transfer metrics
 */

/**
 * Track data transfer event to GoatCounter
 * @param fileSizeBytes - Total file size in bytes that was transferred
 */
export function trackDataTransfer(fileSizeBytes: number): void {
  try {
    // Check if GoatCounter is available
    if (typeof window !== 'undefined' && window.goatcounter) {
      // Send custom event with file size metadata
      window.goatcounter.count({
        event: 'data-transferred',
        data: {
          size_bytes: fileSizeBytes,
          size_mb: Math.round(fileSizeBytes / (1024 * 1024) * 100) / 100 // Convert to MB for easier reading
        }
      })
    }
  } catch (error) {
    // Silently fail - analytics should never break the app
    console.warn('Failed to track data transfer:', error)
  }
}
