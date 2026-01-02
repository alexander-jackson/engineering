-- Create a temporary procedure for structured exercises
CREATE PROCEDURE migrate_structured_exercise(
	variant exercise_variant,
	description TEXT,
	existing TEXT[]
) AS $$
BEGIN
	INSERT INTO structured_exercise (workout_id, variant, description, weight, reps, sets, rpe)
	SELECT workout_id, variant, description, weight, reps, sets, rpe
	FROM exercises
	WHERE name = ANY(existing);
END;
$$ LANGUAGE plpgsql;

-- Create a temporary procedure for freeform exercises
CREATE PROCEDURE migrate_freeform_exercise(
	description TEXT,
	existing TEXT[]
) AS $$
BEGIN
	INSERT INTO freeform_exercise (workout_id, variant, description, weight, reps, sets, rpe)
	SELECT workout_id, 'Other', description, weight, reps, sets, rpe
	FROM exercises
	WHERE name = ANY(existing);
END;
$$ LANGUAGE plpgsql;

-- SQUAT VARIATIONS
CALL migrate_structured_exercise('Squat', '2ct Pause', '{ "2ct Pause Squat" }');
CALL migrate_structured_exercise('Squat', '4.2.0 Tempo', '{ "4.2.0 Tempo Squat" }');
CALL migrate_structured_exercise('Squat', 'Pause', '{ "Paused Squat", "Pause Squat" }');
CALL migrate_structured_exercise('Squat', 'Competition', '{ "Squat" }');
CALL migrate_structured_exercise('Squat', 'Zercher', '{ "Zercher Squat" }');
CALL migrate_structured_exercise('Squat', '3.1.0 Tempo', '{ "Tempo squats" }');
CALL migrate_structured_exercise('Squat', 'Front', '{ "Front Squat" }');

-- SQUAT ACCESSORIES
CALL migrate_freeform_exercise('Belt Squat', '{ "Belt Squat" }');
CALL migrate_freeform_exercise('Bulgarian Split Squat', '{ "Bulgarian Split Squat", "Bulgarian Split Squats" }');

-- BENCH VARIATIONS
CALL migrate_structured_exercise('Bench', '3.1.0 Tempo', '{ "3.1.0 Tempo Bench", "3ct Tempo Bench" }');
CALL migrate_structured_exercise('Bench', '3ct Pause', '{ "3ct Pause Bench" }');
CALL migrate_structured_exercise('Bench', 'Competition', '{ "Bench" }');
CALL migrate_structured_exercise('Bench', 'Close-Grip', '{ "Close Grip Bench" }');
CALL migrate_structured_exercise('Bench', 'Feet-Up Close-Grip', '{ "Close Grip Feet-Up Bench", "Feet-Up Close Grip Bench" }');
CALL migrate_structured_exercise('Bench', 'Incline', '{ "Incline Bench" }');
CALL migrate_structured_exercise('Bench', 'Feet-Up 4.2.0 Tempo', '{ "Tempo Feet-Up Bench" }');
CALL migrate_structured_exercise('Bench', 'Touch and Go', '{ "Touch and Go Bench" }');
CALL migrate_structured_exercise('Bench', 'Spoto', '{ "Spoto Press" }');
CALL migrate_structured_exercise('Bench', 'Larsen', '{ "Larsen Press" }');
CALL migrate_structured_exercise('Bench', 'Feet-Up', '{ "Feet-Up Bench" }');
CALL migrate_structured_exercise('Bench', '15s Hold', '{ "Bench Hold 15 sec" }');

-- BENCH ACCESSORIES
CALL migrate_freeform_exercise('Incline Dumbbell Press', '{ "Incline DB Press", "Incline Dumbbell Press" }');
CALL migrate_freeform_exercise('Dumbbell Press', '{ "Flat DB Press" }');
CALL migrate_freeform_exercise('Seated Dumbbell Press', '{ "Seated DB Press" }');

-- DEADLIFT VARIATIONS
CALL migrate_structured_exercise('Deadlift', 'Conventional', '{ "Conventional Deadlift" }');
CALL migrate_structured_exercise('Deadlift', 'Competition', '{ "Deadlift" }');
CALL migrate_structured_exercise('Deadlift', 'Pause', '{ "Paused Deadlift" }');
CALL migrate_structured_exercise('Deadlift', 'Stiff-Leg', '{ "Stiff Leg Deadlift" }');
CALL migrate_structured_exercise('Deadlift', 'Sumo', '{ "Sumo Deadlift" }');
CALL migrate_structured_exercise('Deadlift', 'Halting', '{ "Halting Sumo" }');

-- DEADLIFT ACCESSORIES
CALL migrate_freeform_exercise('Dumbbell RDL', '{ "DB Romanian Deadlift", "Dumbbell RDLs" }');

-- GENERAL ACCESSORIES
CALL migrate_freeform_exercise('Pendlay Row', '{ "Pendlay Row", "Pendlay Rows" }');
CALL migrate_freeform_exercise('Single Arm Dumbbell Row', '{ "Single Arm DB Row", "Single Arm Dumbbell Rows" }');
CALL migrate_freeform_exercise('Overhead Press', '{ "Strict Press", "Overhead Press" }');
CALL migrate_freeform_exercise('Chest-Supported Dumbbell Row', '{ "Chest Supported DB Row" }');
CALL migrate_freeform_exercise('Walking Lunges', '{ "DB Walking Lunges" }');
CALL migrate_freeform_exercise('Leg Press', '{ "Leg Press" }');
CALL migrate_freeform_exercise('Lat Pulldown', '{ "Lat Pulldown", "Lat Pull Down" }');
CALL migrate_freeform_exercise('Reverse Hyper-Extension', '{ "Reverse Hypers" }');

-- Drop the procedures
DROP PROCEDURE migrate_structured_exercise;
DROP PROCEDURE migrate_freeform_exercise;
