echo "Fetching secrets from 1Password..."
export TF_VAR_db_password
TF_VAR_db_password=$(op item get --vault Private "Database Passwords" --fields "postgres" --reveal)
echo "Secrets fetched successfully."
