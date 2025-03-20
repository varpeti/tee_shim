import time

# Get the start time
start_time = time.time()
# Loop for 20 seconds
while time.time() - start_time < 3:
    # Get the current time
    current_time = time.strftime("%H:%M:%S", time.localtime())

    # Print the current time and overwrite the previous output
    print(f"\rCurrent Time: {current_time}", end="", flush=True)

    # Wait for a second before updating the time
    time.sleep(1)
# After 20 seconds, print a newline for clean output
print("\nDone!")
