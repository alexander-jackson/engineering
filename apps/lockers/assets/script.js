// Keypad state
let keypadValue = '';

// Update the keypad display
function updateKeypadDisplay() {
    const display = document.getElementById('keypad-display');
    const hiddenInput = document.getElementById('locker_number');

    if (keypadValue === '') {
        display.textContent = '---';
        display.classList.remove('text-amber-400');
        display.classList.add('text-gray-600');
        hiddenInput.value = '';
    } else {
        display.textContent = keypadValue.padStart(3, '_');
        display.classList.remove('text-gray-600');
        display.classList.add('text-amber-400');
        hiddenInput.value = keypadValue;
    }
}

// Handle keypad button press
function keypadPress(digit) {
    // Only allow 3 digits max
    if (keypadValue.length < 3) {
        keypadValue += digit;
        updateKeypadDisplay();

        // Add visual feedback
        const buttons = document.querySelectorAll('.keypad-btn');
        buttons.forEach(btn => {
            if (btn.textContent.trim() === digit) {
                btn.classList.add('pressed');
                setTimeout(() => btn.classList.remove('pressed'), 200);
            }
        });
    }
}

// Clear all input
function keypadClear() {
    keypadValue = '';
    updateKeypadDisplay();
}

// Backspace - remove last digit
function keypadBackspace() {
    keypadValue = keypadValue.slice(0, -1);
    updateKeypadDisplay();
}

// Handle physical keyboard input
document.addEventListener('keydown', function(event) {
    // Only handle if we're on the form page
    if (!document.getElementById('keypad-display')) return;

    if (event.key >= '0' && event.key <= '9') {
        event.preventDefault();
        keypadPress(event.key);
    } else if (event.key === 'Backspace') {
        event.preventDefault();
        keypadBackspace();
    } else if (event.key === 'Escape') {
        event.preventDefault();
        keypadClear();
    }
});

// Form validation
function validateForm(event) {
    const lockerNumber = document.getElementById('locker_number').value;

    if (lockerNumber === '') {
        alert('Please enter a locker number using the keypad');
        event.preventDefault();
        return false;
    }

    const number = parseInt(lockerNumber, 10);

    if (isNaN(number) || number < 0 || number > 999) {
        alert('Please enter a valid locker number between 0 and 999');
        event.preventDefault();
        return false;
    }

    return true;
}

// Confirmation dialog for checkout
function confirmCheckout(event, lockerNumber) {
    const confirmed = confirm(`Are you sure you want to check out locker #${lockerNumber}?`);

    if (!confirmed) {
        event.preventDefault();
        return false;
    }

    return true;
}

// Initialize keypad on page load
document.addEventListener('DOMContentLoaded', function() {
    updateKeypadDisplay();
});
