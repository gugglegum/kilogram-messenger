Kilogram offline publication-conflict signer/viewer
====================================================

This package is intended for a dedicated offline Windows account or computer.
It contains no network, runtime, message-store, session, transport or GUI code.
It never creates an Account Root and never modifies an existing Root directory.

Recommended ceremony
--------------------

1. Copy the Device-signed .pcrq request onto removable media.
2. Inspect it without loading Root:

   .\kilogram-offline.exe inspect-request --request-file .\request.pcrq --qr-output-file .\request.png

3. Compare the complete KPC1 code and artifact digest through an independent
   channel. Do not proceed if any character differs.
4. Sign the exact request. The request, QR and response must be outside the
   Account Root directory:

   .\kilogram-offline.exe authorize --account-dir D:\KilogramRoot --request-file .\request.pcrq --confirm-code KPC1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX --verification-qr-file .\request.png --output-file .\response.pcrp

5. Inspect the response before returning it to the online computer:

   .\kilogram-offline.exe inspect-response --response-file .\response.pcrp --qr-output-file .\response.png

The KPC1 code is a comparison checksum, not a password. Malware controlling
the offline computer, display, keyboard or removable media can still steal the
Root or mislead the operator. Keep the offline OS and Root backup protected.
